package com.example.demo

import com.example.demo.views.ClientView
import com.example.demo.views.SplashView
import tornadofx.*

class FTPClient: App(SplashView::class) {

    companion object {
        const val APP_NAME: String = "FTP Client"
        const val WINDOW_WIDTH = 1280.0
        const val WINDOW_HEIGHT = 720.0
    }

}