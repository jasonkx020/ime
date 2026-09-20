package com.yc.input.speech

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.Log
import java.util.Locale

/**
 * SpeechHost：优先系统 SpeechRecognizer；不可用时回退 stub（演示文案）。
 * 架构上属 App 内置 ASR，不进语言包 OTA。
 */
class SpeechHost(private val context: Context) {
    interface Callback {
        fun onPartial(text: String)
        fun onFinal(text: String)
        fun onError(message: String)
    }

    private val main = Handler(Looper.getMainLooper())
    private var recognizer: SpeechRecognizer? = null

    fun isAvailable(): Boolean = SpeechRecognizer.isRecognitionAvailable(context)

    fun start(langTag: String, callback: Callback) {
        if (!isAvailable()) {
            main.postDelayed({
                callback.onFinal(stubText(langTag))
            }, 400)
            return
        }
        stop()
        val r = SpeechRecognizer.createSpeechRecognizer(context)
        recognizer = r
        r.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) {}
            override fun onBeginningOfSpeech() {}
            override fun onRmsChanged(rmsdB: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() {}
            override fun onError(error: Int) {
                Log.w(TAG, "asr error=$error, fallback stub")
                callback.onFinal(stubText(langTag))
            }
            override fun onResults(results: Bundle?) {
                val list = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                callback.onFinal(list?.firstOrNull()?.takeIf { it.isNotBlank() } ?: stubText(langTag))
            }
            override fun onPartialResults(partialResults: Bundle?) {
                val list = partialResults?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                val t = list?.firstOrNull()
                if (!t.isNullOrBlank()) callback.onPartial(t)
            }
            override fun onEvent(eventType: Int, params: Bundle?) {}
        })
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, localeFor(langTag))
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 3)
        }
        try {
            r.startListening(intent)
        } catch (e: Exception) {
            Log.w(TAG, "startListening failed", e)
            callback.onFinal(stubText(langTag))
        }
    }

    fun stopListening() {
        try {
            recognizer?.stopListening()
        } catch (_: Exception) {
        }
    }

    fun stop() {
        try {
            recognizer?.cancel()
            recognizer?.destroy()
        } catch (_: Exception) {
        }
        recognizer = null
    }

    private fun localeFor(langTag: String): String = when (langTag) {
        "en" -> Locale.US.toLanguageTag()
        "vi" -> "vi-VN"
        "th" -> "th-TH"
        else -> Locale.SIMPLIFIED_CHINESE.toLanguageTag()
    }

    private fun stubText(langTag: String): String = when (langTag) {
        "en" -> "Hello"
        "vi" -> "Xin chào"
        "th" -> "สวัสดี"
        else -> "你好"
    }

    companion object {
        private const val TAG = "SpeechHost"
    }
}
