package com.yc.input

import android.content.Context
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView

/** 主 App 视觉常量（对齐演示稿 / samsung-light）。 */
object AppUi {
    const val BG = 0xFFF2F3F5.toInt()
    const val SURFACE = 0xFFFFFFFF.toInt()
    const val INK = 0xFF202124.toInt()
    const val MUTED = 0xFF5F6368.toInt()
    const val LINE = 0xFFE3E5E8.toInt()
    const val ACCENT = 0xFF1A73E8.toInt()
    const val ACCENT_SOFT = 0xFFE8F0FE.toInt()
    const val OK = 0xFF188038.toInt()
    const val OK_SOFT = 0xFFE6F4EA.toInt()
    const val WARM_SOFT = 0xFFFCE8E6.toInt()
    const val WARM = 0xFFC5221F.toInt()

    fun dp(ctx: Context, v: Int): Int =
        (v * ctx.resources.displayMetrics.density).toInt()

    fun title(ctx: Context, text: String): TextView =
        TextView(ctx).apply {
            this.text = text
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(INK)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 22f)
        }

    fun subtitle(ctx: Context, text: String): TextView =
        TextView(ctx).apply {
            this.text = text
            setTextColor(MUTED)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            setPadding(0, dp(ctx, 4), 0, 0)
        }

    fun sectionLabel(ctx: Context, text: String): TextView =
        TextView(ctx).apply {
            this.text = text
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(MUTED)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
            setPadding(0, dp(ctx, 12), 0, dp(ctx, 8))
        }

    fun card(ctx: Context): LinearLayout =
        LinearLayout(ctx).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(SURFACE)
                cornerRadius = dp(ctx, 12).toFloat()
                setStroke(dp(ctx, 1), LINE)
            }
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
            lp.bottomMargin = dp(ctx, 12)
            layoutParams = lp
        }

    fun primaryButton(ctx: Context, label: String, onClick: () -> Unit): TextView =
        TextView(ctx).apply {
            text = label
            gravity = Gravity.CENTER
            setTextColor(0xFFFFFFFF.toInt())
            typeface = Typeface.DEFAULT_BOLD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(ctx, 16), dp(ctx, 14), dp(ctx, 16), dp(ctx, 14))
            background = GradientDrawable().apply {
                setColor(ACCENT)
                cornerRadius = dp(ctx, 12).toFloat()
            }
            setOnClickListener { onClick() }
        }

    fun secondaryButton(ctx: Context, label: String, onClick: () -> Unit): TextView =
        TextView(ctx).apply {
            text = label
            gravity = Gravity.CENTER
            setTextColor(ACCENT)
            typeface = Typeface.DEFAULT_BOLD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
            setPadding(dp(ctx, 16), dp(ctx, 14), dp(ctx, 16), dp(ctx, 14))
            background = GradientDrawable().apply {
                setColor(SURFACE)
                cornerRadius = dp(ctx, 12).toFloat()
                setStroke(dp(ctx, 1), ACCENT)
            }
            setOnClickListener { onClick() }
        }

    fun iconBox(ctx: Context, label: String, bg: Int = ACCENT_SOFT, fg: Int = ACCENT): TextView =
        TextView(ctx).apply {
            text = label
            gravity = Gravity.CENTER
            setTextColor(fg)
            typeface = Typeface.DEFAULT_BOLD
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            background = GradientDrawable().apply {
                setColor(bg)
                cornerRadius = dp(ctx, 10).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(dp(ctx, 36), dp(ctx, 36))
        }

    fun listRow(
        ctx: Context,
        icon: String,
        title: String,
        desc: String,
        trailing: View? = null,
        iconBg: Int = ACCENT_SOFT,
        iconFg: Int = ACCENT,
        showDivider: Boolean = true,
        onClick: (() -> Unit)? = null,
    ): LinearLayout =
        LinearLayout(ctx).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(ctx, 14), dp(ctx, 14), dp(ctx, 14), dp(ctx, 14))
            if (showDivider) {
                background = GradientDrawable().apply {
                    setColor(SURFACE)
                    setStroke(0, LINE)
                }
            }
            addView(iconBox(ctx, icon, iconBg, iconFg))
            addView(
                LinearLayout(ctx).apply {
                    orientation = LinearLayout.VERTICAL
                    setPadding(dp(ctx, 12), 0, dp(ctx, 8), 0)
                    addView(TextView(ctx).apply {
                        text = title
                        setTextColor(INK)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                    })
                    addView(TextView(ctx).apply {
                        text = desc
                        setTextColor(MUTED)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                        setPadding(0, dp(ctx, 2), 0, 0)
                    })
                },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
            )
            if (trailing != null) addView(trailing)
            if (onClick != null) setOnClickListener { onClick() }
        }

    fun badge(ctx: Context, text: String, on: Boolean): TextView =
        TextView(ctx).apply {
            this.text = text
            setTextColor(if (on) OK else MUTED)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 11f)
            typeface = Typeface.DEFAULT_BOLD
            setPadding(dp(ctx, 8), dp(ctx, 2), dp(ctx, 8), dp(ctx, 2))
            background = GradientDrawable().apply {
                setColor(if (on) OK_SOFT else 0xFFF1F3F4.toInt())
                cornerRadius = dp(ctx, 999).toFloat()
            }
        }

    fun chipLink(ctx: Context, left: String, right: String, onClick: () -> Unit): LinearLayout =
        LinearLayout(ctx).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(ctx, 12), dp(ctx, 10), dp(ctx, 12), dp(ctx, 10))
            background = GradientDrawable().apply {
                setColor(SURFACE)
                cornerRadius = dp(ctx, 8).toFloat()
                setStroke(dp(ctx, 1), LINE)
            }
            addView(
                TextView(ctx).apply {
                    text = left
                    setTextColor(INK)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
            )
            addView(TextView(ctx).apply {
                text = right
                setTextColor(MUTED)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
            })
            setOnClickListener { onClick() }
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            )
            lp.bottomMargin = dp(ctx, 14)
            layoutParams = lp
        }

    fun divider(ctx: Context): View =
        View(ctx).apply {
            setBackgroundColor(LINE)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(ctx, 1),
            )
        }
}
