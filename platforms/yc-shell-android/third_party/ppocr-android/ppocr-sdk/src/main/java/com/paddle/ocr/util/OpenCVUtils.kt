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

package com.paddle.ocr.util

import android.content.Context
import android.util.Log
import org.opencv.android.OpenCVLoader

object OpenCVUtils {

    private var initialized = false

    @Synchronized
    fun init(context: Context): Boolean {
        if (initialized) return true
        // Prefer OpenCVLoader (handles packaging); fall back to direct loadLibrary.
        try {
            if (OpenCVLoader.initDebug()) {
                initialized = true
                Log.i(TAG, "OpenCV loaded via OpenCVLoader.initDebug()")
                return true
            }
            Log.w(TAG, "OpenCVLoader.initDebug() returned false; trying loadLibrary")
        } catch (t: Throwable) {
            Log.w(TAG, "OpenCVLoader.initDebug() failed: ${t.message}")
        }
        try {
            System.loadLibrary("opencv_java4")
            initialized = true
            Log.i(TAG, "OpenCV loaded via System.loadLibrary(opencv_java4)")
            return true
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "Failed to load OpenCV native lib", e)
        }
        return false
    }

    private const val TAG = "OpenCVUtils"
}
