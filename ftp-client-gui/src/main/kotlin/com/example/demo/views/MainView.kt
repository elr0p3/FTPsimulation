package com.example.demo.views

import com.example.demo.app.old.Styles
import tornadofx.*

class MainView : View("FTP Client") {
    override val root = hbox {
        label(title) {
            addClass(Styles.heading)
        }
    }
}