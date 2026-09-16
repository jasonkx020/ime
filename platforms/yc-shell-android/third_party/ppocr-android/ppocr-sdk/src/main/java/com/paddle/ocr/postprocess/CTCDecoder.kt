// Copyright (c) 2026 PaddlePaddle Authors. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package com.paddle.ocr.postprocess

/**
 * CTC greedy decode plus per-timestep top alternatives (for IME candidate pools).
 */
object CTCDecoder {
    private const val BLANK_IDX = 0
    /** Top non-blank classes kept from each CTC timestep. */
    private const val ALT_PER_STEP = 20
    /** Max unique alternative characters returned per crop. */
    private const val ALT_POOL_CAP = 200

    data class DecodeItem(
        val text: String,
        val confidence: Float,
        /** Possible characters from CTC logits, sorted by score descending. */
        val alternatives: List<Pair<String, Float>>,
    )

    fun decode(output: FloatArray, shape: LongArray, characterList: List<String>): List<DecodeItem> {
        val batchSize = shape[0].toInt()
        val timeSteps = shape[1].toInt()
        val numClasses = shape[2].toInt()

        val results = mutableListOf<DecodeItem>()
        for (b in 0 until batchSize) {
            val baseOffset = b * timeSteps * numClasses

            val indices = IntArray(timeSteps)
            val probs = FloatArray(timeSteps)
            val altScores = HashMap<String, Float>()

            for (t in 0 until timeSteps) {
                val offset = baseOffset + t * numClasses
                var maxIdx = 0
                var maxVal = output[offset]
                // Collect top-ALT_PER_STEP non-blank (class index, score)
                val top = ArrayList<Pair<Int, Float>>(ALT_PER_STEP)
                for (c in 0 until numClasses) {
                    val v = output[offset + c]
                    if (v > maxVal) {
                        maxVal = v
                        maxIdx = c
                    }
                    if (c == BLANK_IDX) continue
                    insertTop(top, c to v, ALT_PER_STEP)
                }
                indices[t] = maxIdx
                probs[t] = maxVal
                for ((cls, score) in top) {
                    val charIdx = cls - 1
                    if (charIdx < 0 || charIdx >= characterList.size) continue
                    val ch = characterList[charIdx]
                    if (ch.isEmpty()) continue
                    val prev = altScores[ch]
                    if (prev == null || score > prev) {
                        altScores[ch] = score
                    }
                }
            }

            val keptProbs = mutableListOf<Float>()
            val sb = StringBuilder()
            var prevIdx = -1
            for (t in 0 until timeSteps) {
                val idx = indices[t]
                if (idx != BLANK_IDX && idx != prevIdx) {
                    val charIdx = idx - 1
                    if (charIdx >= 0 && charIdx < characterList.size) {
                        sb.append(characterList[charIdx])
                        keptProbs.add(probs[t])
                    }
                }
                prevIdx = idx
            }

            val confidence = if (keptProbs.isNotEmpty()) keptProbs.average().toFloat() else 0f
            val alternatives = altScores.entries
                .sortedByDescending { it.value }
                .take(ALT_POOL_CAP)
                .map { it.key to it.value }
            results.add(DecodeItem(sb.toString(), confidence, alternatives))
        }
        return results
    }

    /** Keep [list] as the top-[limit] pairs by second (score). */
    private fun insertTop(list: ArrayList<Pair<Int, Float>>, item: Pair<Int, Float>, limit: Int) {
        if (list.size < limit) {
            list.add(item)
            if (list.size == limit) {
                list.sortBy { it.second }
            }
            return
        }
        if (item.second <= list[0].second) return
        list[0] = item
        // Restore min-at-front for next compare (small k, fine to re-sort)
        list.sortBy { it.second }
    }
}
