package com.yc.input.llm

import android.content.Context
import android.util.Log
import org.json.JSONObject
import java.io.File
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/**
 * 选词习惯日记：热路径只入队；专用线程 flush，绝不反压输入。
 */
object HabitJournal {
    private const val TAG = "HabitJournal"
    private const val FILE = "habit_journal.jsonl"
    private const val MAX_QUEUE = 256
    private const val MAX_FILE_LINES = 4000

    private val queue = ConcurrentLinkedQueue<String>()
    private val queued = AtomicInteger(0)
    private val flushScheduled = AtomicBoolean(false)
    private val writer = Executors.newSingleThreadExecutor { r ->
        Thread(r, "habit-journal").apply { isDaemon = true }
    }

    data class Event(
        val lang: String,
        val queryKey: String,
        val selectedWord: String,
        val candidatePos: Int,
        val ts: Long = System.currentTimeMillis(),
    )

    /** 热路径：微秒级入队；队列满丢最旧。 */
    fun enqueue(ctx: Context, event: Event) {
        if (event.queryKey.isBlank() || event.selectedWord.isBlank()) return
        val line = JSONObject()
            .put("lang", event.lang)
            .put("query_key", event.queryKey.take(64))
            .put("selected_word", event.selectedWord.take(64))
            .put("candidate_pos", event.candidatePos.coerceIn(0, 999))
            .put("ts", event.ts)
            .toString()
        while (queued.get() >= MAX_QUEUE) {
            if (queue.poll() != null) queued.decrementAndGet() else break
        }
        queue.offer(line)
        queued.incrementAndGet()
        scheduleFlush(ctx.applicationContext)
    }

    private fun scheduleFlush(appCtx: Context) {
        if (!flushScheduled.compareAndSet(false, true)) return
        writer.execute {
            try {
                flushNow(appCtx)
            } catch (e: Exception) {
                Log.w(TAG, "flush failed", e)
            } finally {
                flushScheduled.set(false)
                if (queued.get() > 0) scheduleFlush(appCtx)
            }
        }
    }

    private fun flushNow(ctx: Context) {
        val batch = ArrayList<String>(64)
        while (batch.size < 64) {
            val line = queue.poll() ?: break
            queued.decrementAndGet()
            batch.add(line)
        }
        if (batch.isEmpty()) return
        val file = File(ctx.filesDir, FILE)
        file.appendText(batch.joinToString(separator = "\n", postfix = "\n"))
        trimIfNeeded(file)
    }

    private fun trimIfNeeded(file: File) {
        if (!file.exists() || file.length() < 512_000) return
        val lines = file.readLines()
        if (lines.size <= MAX_FILE_LINES) return
        file.writeText(lines.takeLast(MAX_FILE_LINES / 2).joinToString("\n", postfix = "\n"))
    }

    fun readRecent(ctx: Context, limit: Int = 500): List<Event> {
        val file = File(ctx.filesDir, FILE)
        if (!file.exists()) return emptyList()
        return try {
            file.readLines()
                .asReversed()
                .asSequence()
                .filter { it.isNotBlank() }
                .take(limit)
                .mapNotNull { parseLine(it) }
                .toList()
                .asReversed()
        } catch (e: Exception) {
            Log.w(TAG, "readRecent", e)
            emptyList()
        }
    }

    private fun parseLine(line: String): Event? =
        try {
            val o = JSONObject(line)
            Event(
                lang = o.optString("lang"),
                queryKey = o.optString("query_key"),
                selectedWord = o.optString("selected_word"),
                candidatePos = o.optInt("candidate_pos"),
                ts = o.optLong("ts"),
            )
        } catch (_: Exception) {
            null
        }
}
