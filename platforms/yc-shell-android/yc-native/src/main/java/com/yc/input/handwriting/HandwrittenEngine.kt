package com.yc.input.handwriting

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.util.Log
import com.shiyu.handwritten.runtime.HCCRRecognizer
import com.yc.input.ui.HandwritingPad
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt

/**
 * Android wrapper around Ismantic/Handwritten (NCNN INT8).
 *
 * Rasterization follows Handwritten's HandwritingView:
 * - render to ~360px canvas (not hi-dpi then crush)
 * - stroke width ≈ 4.4% of canvas (≈16px on 360)
 * - no AA / butt caps for model bitmap (match PIL.ImageDraw.line)
 * - preprocess.c then bbox-crops to 64×64
 */
class HandwrittenEngine(private val context: Context) {
    data class Candidate(val text: String, val score: Float)

    data class RecognizeOutput(
        val candidates: List<Candidate>,
        val recognizedText: String?,
        val needsCloudConfirm: Boolean,
    )

    private var recognizer: HCCRRecognizer? = null
    private val loaded = AtomicBoolean(false)
    private val loadFailed = AtomicBoolean(false)

    fun isReady(): Boolean = loaded.get() && recognizer != null

    @Synchronized
    fun ensureLoaded(): Boolean {
        if (loaded.get()) return recognizer != null
        if (loadFailed.get()) return false
        return try {
            HCCRRecognizer.loadNativeLibrary("hccr_jni")
            val r = HCCRRecognizer(
                context.assets,
                ASSET_PARAM,
                ASSET_BIN,
                ASSET_CHARSET,
            )
            recognizer = r
            loaded.set(true)
            Log.i(TAG, "Handwritten NCNN loaded")
            true
        } catch (t: Throwable) {
            loadFailed.set(true)
            Log.w(TAG, "Handwritten NCNN unavailable", t)
            false
        }
    }

    fun recognizeStrokes(
        strokes: List<HandwritingPad.StrokePayload>,
        continuous: Boolean,
        topK: Int = TOP_K,
    ): RecognizeOutput {
        if (strokes.isEmpty() || !ensureLoaded()) {
            return RecognizeOutput(emptyList(), null, continuous)
        }
        val clusters =
            if (continuous) {
                segment(strokes, GAP_MS, GAP_NORM)
            } else {
                listOf(strokes)
            }
        if (clusters.isEmpty()) {
            return RecognizeOutput(emptyList(), null, continuous)
        }

        val phrase = StringBuilder()
        var lastTop: List<Candidate> = emptyList()
        var minConf = 1f
        for (cluster in clusters) {
            val rendered = rasterizeForModel(cluster) ?: continue
            val (gray, w, h) = rendered
            val top = predictGray(gray, w, h, topK)
            if (top.isEmpty()) continue
            phrase.append(top[0].text)
            minConf = min(minConf, top[0].score)
            lastTop = top
        }
        if (lastTop.isEmpty()) {
            return RecognizeOutput(emptyList(), null, continuous)
        }
        val candidates = LinkedHashMap<String, Float>()
        if (continuous && phrase.isNotEmpty()) {
            candidates[phrase.toString()] = minConf
        }
        for (c in lastTop) {
            candidates.putIfAbsent(c.text, c.score)
        }
        val list = candidates.entries.take(topK).map { Candidate(it.key, it.value) }
        val needsCloud = continuous && minConf < CLOUD_THRESHOLD
        return RecognizeOutput(
            candidates = list,
            recognizedText = if (continuous) phrase.toString().ifEmpty { null } else null,
            needsCloudConfirm = needsCloud,
        )
    }

    private fun predictGray(gray: ByteArray, w: Int, h: Int, k: Int): List<Candidate> {
        val r = recognizer ?: return emptyList()
        val input = FloatArray(64 * 64)
        if (!r.preprocess(gray, w, h, input)) {
            return emptyList()
        }
        return r.predict(input, k).map { Candidate(it.text, it.probability) }
    }

    companion object {
        private const val TAG = "HandwrittenEngine"
        private const val ASSET_PARAM = "models/handwriting/model.ncnn.param"
        private const val ASSET_BIN = "models/handwriting/model.ncnn.bin"
        private const val ASSET_CHARSET = "models/handwriting/charset.json"
        /** Match Handwritten HandwritingView.RENDER_TARGET_SIZE */
        private const val RENDER_TARGET_SIZE = 360
        /** Match Handwritten RENDER_STROKE_RATIO (16px / 360) */
        private const val RENDER_STROKE_RATIO = 0.044f
        private const val GAP_MS = 280L
        private const val GAP_NORM = 0.35f
        private const val CLOUD_THRESHOLD = 0.6f
        const val TOP_K = 30

        fun segment(
            strokes: List<HandwritingPad.StrokePayload>,
            gapMs: Long,
            gapNorm: Float,
        ): List<List<HandwritingPad.StrokePayload>> {
            if (strokes.isEmpty()) return emptyList()
            val out = mutableListOf<MutableList<HandwritingPad.StrokePayload>>()
            var cur = mutableListOf(strokes[0])
            for (i in 1 until strokes.size) {
                val prev = strokes[i - 1]
                val next = strokes[i]
                val prevT = prev.timesMs.lastOrNull() ?: 0L
                val nextT = next.timesMs.firstOrNull() ?: 0L
                val dt = nextT - prevT
                val lastIdx = (prev.timesMs.size - 1).coerceAtLeast(0)
                val px = prev.xyPressure[lastIdx * 3]
                val py = prev.xyPressure[lastIdx * 3 + 1]
                val nx = next.xyPressure[0]
                val ny = next.xyPressure[1]
                val dx = nx - px
                val dy = ny - py
                val dist = sqrt(dx * dx + dy * dy)
                if (dt >= gapMs || dist >= gapNorm) {
                    out.add(cur)
                    cur = mutableListOf()
                }
                cur.add(next)
            }
            if (cur.isNotEmpty()) out.add(cur)
            return out
        }

        /**
         * Render strokes for NCNN:
         * 1) compute ink bbox in normalized space
         * 2) map to a square RENDER_TARGET_SIZE canvas with margin
         * 3) hard black strokes (no AA), width = 4.4% of canvas
         */
        fun rasterizeForModel(
            strokes: List<HandwritingPad.StrokePayload>,
        ): Triple<ByteArray, Int, Int>? {
            if (strokes.isEmpty()) return null
            var minX = 1f
            var minY = 1f
            var maxX = 0f
            var maxY = 0f
            var any = false
            for (stroke in strokes) {
                val n = stroke.timesMs.size
                for (i in 0 until n) {
                    val x = stroke.xyPressure[i * 3]
                    val y = stroke.xyPressure[i * 3 + 1]
                    minX = min(minX, x)
                    minY = min(minY, y)
                    maxX = max(maxX, x)
                    maxY = max(maxY, y)
                    any = true
                }
            }
            if (!any) return null
            // Expand bbox slightly so strokes aren't clipped after thick render.
            val pad = 0.06f
            minX = (minX - pad).coerceAtLeast(0f)
            minY = (minY - pad).coerceAtLeast(0f)
            maxX = (maxX + pad).coerceAtMost(1f)
            maxY = (maxY + pad).coerceAtMost(1f)
            val bw = (maxX - minX).coerceAtLeast(0.05f)
            val bh = (maxY - minY).coerceAtLeast(0.05f)
            val side = max(bw, bh)
            // Center content in square logical space.
            val cx = (minX + maxX) / 2f
            val cy = (minY + maxY) / 2f
            val x0 = cx - side / 2f
            val y0 = cy - side / 2f

            val size = RENDER_TARGET_SIZE
            val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
            val canvas = Canvas(bmp)
            canvas.drawColor(Color.WHITE)
            val paint = Paint().apply {
                color = Color.BLACK
                style = Paint.Style.STROKE
                isAntiAlias = false // hard binary like PIL L-mode line
                strokeCap = Paint.Cap.BUTT
                strokeJoin = Paint.Join.ROUND
                strokeWidth = size * RENDER_STROKE_RATIO
            }

            fun mapX(nx: Float): Float = ((nx - x0) / side).coerceIn(0f, 1f) * (size - 1)
            fun mapY(ny: Float): Float = ((ny - y0) / side).coerceIn(0f, 1f) * (size - 1)

            for (stroke in strokes) {
                val n = stroke.timesMs.size
                if (n < 2) continue
                for (i in 0 until n - 1) {
                    val xA = mapX(stroke.xyPressure[i * 3])
                    val yA = mapY(stroke.xyPressure[i * 3 + 1])
                    val xB = mapX(stroke.xyPressure[(i + 1) * 3])
                    val yB = mapY(stroke.xyPressure[(i + 1) * 3 + 1])
                    canvas.drawLine(xA, yA, xB, yB, paint)
                }
            }

            val pixels = IntArray(size * size)
            bmp.getPixels(pixels, 0, size, 0, 0, size, size)
            bmp.recycle()
            val gray = ByteArray(size * size)
            for (i in pixels.indices) {
                // Force near-binary: dark → 0, light → 255 (matches HWDB white-bg)
                val c = pixels[i]
                val r = (c shr 16) and 0xff
                val g = (c shr 8) and 0xff
                val b = c and 0xff
                val v = (r + g + b) / 3
                gray[i] = if (v < 220) 0 else 255.toByte()
            }
            return Triple(gray, size, size)
        }
    }
}
