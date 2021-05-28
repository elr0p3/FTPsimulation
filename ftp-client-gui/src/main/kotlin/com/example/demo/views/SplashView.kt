package com.example.demo.views

import com.example.demo.FTPClient
import com.example.demo.extensions.makeLabel
import com.example.demo.statics.R
import tornadofx.*
import java.util.*
import kotlin.math.sin

class SplashView: View(FTPClient.APP_NAME.makeLabel(this.LABEL)) {

    override val root = pane {
        prefWidth = FTPClient.WINDOW_WIDTH
        prefHeight = FTPClient.WINDOW_HEIGHT

        imageview(R.images.IMG_FTP) {
            fitHeight = 200.0
            fitWidth = 200.0
            x = (FTPClient.WINDOW_WIDTH / 2) - 100.0
            y = (FTPClient.WINDOW_HEIGHT / 2) - 100.0
        }
    }

    init {
        val delay = (2.5).seconds
        runLater(delay) { replaceWith(ConnectionView::class, ViewTransition.FadeThrough(1.seconds)) }
    }

    companion object {
        const val LABEL: String = "Splash"
    }
}