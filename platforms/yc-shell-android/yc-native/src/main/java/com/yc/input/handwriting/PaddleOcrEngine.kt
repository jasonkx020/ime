package com.yc.input.handwriting

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.util.Log
import com.paddle.ocr.EngineConfig
import com.paddle.ocr.PaddleOCR
import com.paddle.ocr.PaddleOCRConfig
import com.paddle.ocr.util.OpenCVUtils
import com.yc.input.ui.HandwritingPad
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max
import kotlin.math.min
import kotlinx.coroutines.runBlocking

/**
 * PP-OCRv6 (ONNX Runtime via official ppocr-sdk) for handwriting pad.
 * Rasterizes strokes to a white-bg bitmap, runs OCR, maps to CandBar candidates.
 */
class PaddleOcrEngine(private val context: Context) {
    data class Candidate(val text: String, val score: Float)

    data class RecognizeOutput(
        val candidates: List<Candidate>,
        val recognizedText: String?,
        val needsCloudConfirm: Boolean,
    )

    private var ocr: PaddleOCR? = null
    private val loaded = AtomicBoolean(false)
    private val loadFailed = AtomicBoolean(false)

    fun isReady(): Boolean = loaded.get() && ocr != null

    @Synchronized
    fun ensureLoaded(): Boolean {
        if (loaded.get()) return ocr != null
        if (loadFailed.get()) return false
        return try {
            if (!OpenCVUtils.init(context.applicationContext)) {
                // Do not latch forever: OpenCV .so may appear after first install race.
                Log.w(TAG, "OpenCV init failed (will retry next time)")
                return false
            }
            val engine = runBlocking {
                PaddleOCR.create(
                    context = context.applicationContext,
                    config = PaddleOCRConfig(
                        detLimitSideLen = 64,
                        detLimitType = "min",
                        detThresh = 0.3f,
                        detBoxThresh = 0.5f,
                        detUnclipRatio = 1.5f,
                        recScoreThresh = 0.0f,
                        recBatchSize = 1,
                    ),
                    engineConfig = EngineConfig(numThreads = 4),
                    detModelAssetPath = DET_ASSET,
                    recModelAssetPath = REC_ASSET,
                    recConfigAssetPath = REC_YML_ASSET,
                )
            }
            ocr = engine
            loaded.set(true)
            loadFailed.set(false)
            Log.i(TAG, "PP-OCRv6 ONNX loaded (cold=${engine.coldLoadTimeMs}ms)")
            true
        } catch (t: Throwable) {
            loadFailed.set(true)
            Log.w(TAG, "PP-OCRv6 unavailable", t)
            false
        }
    }

    /** True if a previous model init threw (not mere OpenCV miss). */
    fun hasHardFailure(): Boolean = loadFailed.get() && !loaded.get()

    fun recognizeStrokes(
        strokes: List<HandwritingPad.StrokePayload>,
        continuous: Boolean,
        topK: Int = TOP_K,
    ): RecognizeOutput {
        if (strokes.isEmpty() || !ensureLoaded()) {
            return RecognizeOutput(emptyList(), null, continuous)
        }
        val engine = ocr ?: return RecognizeOutput(emptyList(), null, continuous)
        val bitmap = rasterizeBitmap(strokes) ?: return RecognizeOutput(emptyList(), null, continuous)
        return try {
            val result = runBlocking { engine.recognize(bitmap) }
            mapResult(result.results.map { it.text to it.confidence }, continuous, topK)
        } catch (t: Throwable) {
            Log.w(TAG, "PP-OCRv6 recognize failed", t)
            RecognizeOutput(emptyList(), null, continuous)
        } finally {
            if (!bitmap.isRecycled) bitmap.recycle()
        }
    }

    fun release() {
        val engine = ocr
        ocr = null
        loaded.set(false)
        if (engine != null) {
            try {
                runBlocking { engine.release() }
            } catch (t: Throwable) {
                Log.w(TAG, "PP-OCRv6 release failed", t)
            }
        }
    }

    private fun mapResult(
        items: List<Pair<String, Float>>,
        continuous: Boolean,
        topK: Int,
    ): RecognizeOutput {
        val ordered = LinkedHashMap<String, Float>()
        var minConf = 1f
        for ((text, confRaw) in items) {
            val label = text.trim()
            if (label.isEmpty()) continue
            val conf = confRaw.coerceIn(0f, 1f).let { if (it <= 0f) 1f else it }
            minConf = min(minConf, conf)
            ordered.putIfAbsent(label, conf)
            if (label.length > 1) {
                for (ch in label) {
                    if (!ch.isWhitespace()) {
                        ordered.putIfAbsent(ch.toString(), conf * 0.95f)
                    }
                }
            }
        }
        val joined = items.map { it.first.trim() }.filter { it.isNotEmpty() }.joinToString("")
        if (joined.isNotEmpty()) {
            val rest = LinkedHashMap<String, Float>()
            rest[joined] = minConf
            for ((k, v) in ordered) {
                if (k != joined) rest[k] = v
            }
            ordered.clear()
            ordered.putAll(rest)
            if (!continuous && joined.length > 1) {
                for (ch in joined) {
                    if (!ch.isWhitespace()) {
                        ordered.putIfAbsent(ch.toString(), minConf * 0.9f)
                    }
                }
            }
        }

        val list = ordered.entries.take(topK).map { Candidate(it.key, it.value) }
        if (list.isEmpty()) {
            return RecognizeOutput(emptyList(), null, continuous)
        }
        val needsCloud = continuous && minConf < CLOUD_THRESHOLD
        return RecognizeOutput(
            candidates = list,
            recognizedText = if (continuous) joined.ifEmpty { null } else null,
            needsCloudConfirm = needsCloud,
        )
    }

    companion object {
        private const val TAG = "PaddleOcrEngine"
        private const val DET_ASSET = "models/ppocr/det/inference.onnx"
        private const val REC_ASSET = "models/ppocr/rec/inference.onnx"
        private const val REC_YML_ASSET = "models/ppocr/rec/inference.yml"
        private const val RENDER_TARGET_SIZE = 360
        private const val RENDER_STROKE_RATIO = 0.044f
        private const val CLOUD_THRESHOLD = 0.6f
        const val TOP_K = 30

        fun rasterizeBitmap(strokes: List<HandwritingPad.StrokePayload>): Bitmap? {
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
            val pad = 0.08f
            minX = (minX - pad).coerceAtLeast(0f)
            minY = (minY - pad).coerceAtLeast(0f)
            maxX = (maxX + pad).coerceAtMost(1f)
            maxY = (maxY + pad).coerceAtMost(1f)
            val bw = (maxX - minX).coerceAtLeast(0.05f)
            val bh = (maxY - minY).coerceAtLeast(0.05f)
            val side = max(bw, bh)
            val cx = (minX + maxX) / 2f
            val cy = (minY + maxY) / 2f
            val x0 = cx - side / 2f
            val y0 = cy - side / 2f

            val size = RENDER_TARGET_SIZE
            val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
            val canvas = Canvas(bmp)
            canvas.drawColor(Color.WHITE)
            val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.BLACK
                style = Paint.Style.STROKE
                strokeCap = Paint.Cap.ROUND
                strokeJoin = Paint.Join.ROUND
                strokeWidth = size * RENDER_STROKE_RATIO
            }
            fun mapX(nx: Float): Float = ((nx - x0) / side).coerceIn(0f, 1f) * (size - 1)
            fun mapY(ny: Float): Float = ((ny - y0) / side).coerceIn(0f, 1f) * (size - 1)
            for (stroke in strokes) {
                val n = stroke.timesMs.size
                if (n < 2) continue
                for (i in 0 until n - 1) {
                    canvas.drawLine(
                        mapX(stroke.xyPressure[i * 3]),
                        mapY(stroke.xyPressure[i * 3 + 1]),
                        mapX(stroke.xyPressure[(i + 1) * 3]),
                        mapY(stroke.xyPressure[(i + 1) * 3 + 1]),
                        paint,
                    )
                }
            }
            return bmp
        }
    }
}
