package com.yc.input.llm

import android.content.Context
import android.util.Log
import com.yc.input.native.YcNative
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/**
 * 选词画像流水线：单飞后台；热路径只计数/触发判定。
 */
object HabitPersonaPipeline {
    private const val TAG = "HabitPersona"
    private const val PREF = "yc_habit_persona"
    private const val KEY_SELECTS_SINCE = "selects_since_opt"
    private const val KEY_LAST_OPT_MS = "last_opt_ms"
    private const val KEY_LAST_SUMMARY = "last_summary"
    private const val SETTINGS_PREF = "yc_settings"
    private const val KEY_PERSONALIZATION = "personalization"

    private const val THRESHOLD_WITH_KEY = 30
    private const val THRESHOLD_NO_KEY = 100
    private const val MIN_INTERVAL_MS = 24L * 60 * 60 * 1000

    private val running = AtomicBoolean(false)
    private val selectSince = AtomicInteger(0)
    private val executor = Executors.newSingleThreadExecutor { r ->
        Thread(r, "habit-persona").apply { isDaemon = true }
    }

    fun personalizationEnabled(ctx: Context): Boolean =
        ctx.getSharedPreferences(SETTINGS_PREF, Context.MODE_PRIVATE)
            .getBoolean(KEY_PERSONALIZATION, true)

    /** 热路径：选词成功后调用；仅计数 + 可能调度后台任务。 */
    fun onSelect(ctx: Context, lang: String, queryKey: String, word: String, candidatePos: Int) {
        if (!personalizationEnabled(ctx)) return
        val app = ctx.applicationContext
        HabitJournal.enqueue(
            app,
            HabitJournal.Event(
                lang = lang,
                queryKey = queryKey,
                selectedWord = word,
                candidatePos = candidatePos,
            ),
        )
        val n = selectSince.incrementAndGet()
        prefs(app).edit().putInt(KEY_SELECTS_SINCE, n).apply()
        maybeSchedule(app, force = false)
    }

    fun maybeSchedule(ctx: Context, force: Boolean) {
        if (!personalizationEnabled(ctx)) return
        val app = ctx.applicationContext
        if (!force) {
            val hasKey = LlmByokPrefs.resolveEndpoint(app, LlmProviderCatalog.load(app)) != null
            val threshold = if (hasKey) THRESHOLD_WITH_KEY else THRESHOLD_NO_KEY
            val n = selectSince.get().coerceAtLeast(prefs(app).getInt(KEY_SELECTS_SINCE, 0))
            val last = prefs(app).getLong(KEY_LAST_OPT_MS, 0L)
            val dueByCount = n >= threshold
            val dueByTime = last > 0 && System.currentTimeMillis() - last >= MIN_INTERVAL_MS && n >= 5
            if (!dueByCount && !dueByTime) return
        }
        if (!running.compareAndSet(false, true)) {
            Log.i(TAG, "skip: already running")
            return
        }
        executor.execute {
            try {
                runOptimize(app)
            } catch (e: Exception) {
                Log.w(TAG, "optimize failed", e)
            } finally {
                running.set(false)
            }
        }
    }

    fun optimizeNowAsync(ctx: Context, onDone: ((HabitPersonaPack) -> Unit)? = null) {
        val app = ctx.applicationContext
        if (!running.compareAndSet(false, true)) {
            onDone?.invoke(
                HabitPersonaPack(error = "优化进行中，请稍候"),
            )
            return
        }
        executor.execute {
            var pack = HabitPersonaPack(error = "unknown")
            try {
                pack = runOptimize(app)
            } catch (e: Exception) {
                Log.w(TAG, "optimizeNow failed", e)
                pack = HabitPersonaPack(error = e.message)
            } finally {
                running.set(false)
            }
            if (onDone != null) {
                android.os.Handler(android.os.Looper.getMainLooper()).post { onDone(pack) }
            }
        }
    }

    fun lastSummary(ctx: Context): String =
        prefs(ctx).getString(KEY_LAST_SUMMARY, "") ?: ""

    fun isRunning(): Boolean = running.get()

    private fun runOptimize(ctx: Context): HabitPersonaPack {
        val lang = guessLang(ctx)
        val snap = HabitPersonaLlmRouter.buildSnapshot(ctx, lang)
        if (snap.topWords.isEmpty() && snap.topKeys.isEmpty()) {
            Log.i(TAG, "empty snapshot, skip apply")
            return HabitPersonaPack(error = "暂无足够选词数据")
        }
        val pack = HabitPersonaLlmRouter.analyze(ctx, snap)
        applyPack(pack)
        selectSince.set(0)
        prefs(ctx).edit()
            .putInt(KEY_SELECTS_SINCE, 0)
            .putLong(KEY_LAST_OPT_MS, System.currentTimeMillis())
            .putString(KEY_LAST_SUMMARY, pack.summary() + if (pack.local) " ·local" else " ·llm")
            .apply()
        Log.i(TAG, "applied ${pack.summary()} err=${pack.error}")
        return pack
    }

    private fun applyPack(pack: HabitPersonaPack) {
        // 坏包 / 全空：保留上一版（FFI 不换表）
        if (pack.preferPairs.length() == 0 &&
            pack.demote.length() == 0 &&
            pack.boosts.length() == 0
        ) {
            return
        }
        try {
            val rc = YcNative.ycPersonalizationApply(
                pack.pairsJson(),
                pack.deltasJson(),
                pack.boostsJson(),
            )
            if (rc != YcNative.OK) {
                Log.w(TAG, "ycPersonalizationApply rc=$rc")
            }
        } catch (e: UnsatisfiedLinkError) {
            Log.w(TAG, "FFI missing, pack not applied to intel", e)
        } catch (e: Exception) {
            Log.w(TAG, "applyPack", e)
        }
    }

    /** 启动 IME 时尝试把磁盘上的 pack 再 apply 一次（core 也会自载；双保险）。 */
    fun reloadPersisted(ctx: Context) {
        executor.execute {
            try {
                val dir = ctx.filesDir
                val pairsFile = java.io.File(dir, "prefer_pairs.json")
                val deltasFile = java.io.File(dir, "score_deltas.json")
                val pairs = pairsFile.takeIf { it.exists() }?.readText()
                val deltas = deltasFile.takeIf { it.exists() }?.readText()
                if (pairs.isNullOrBlank() && deltas.isNullOrBlank()) return@execute
                YcNative.ycPersonalizationApply(pairs ?: "[]", deltas ?: "[]", "[]")
            } catch (e: Exception) {
                Log.w(TAG, "reloadPersisted", e)
            }
        }
    }

    private fun guessLang(ctx: Context): String =
        ctx.getSharedPreferences("yc_lang", Context.MODE_PRIVATE)
            .getString("preferred_lang", "zh") ?: "zh"

    private fun prefs(ctx: Context) =
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE)
}
