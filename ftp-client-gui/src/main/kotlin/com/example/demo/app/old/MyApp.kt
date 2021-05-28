package com.example.demo.app.old

import com.example.demo.views.MainView
import tornadofx.App

class MyApp: App(MainView::class, Styles::class) {
    companion object {
        val NAME: String = "FTP Client"
    }
}