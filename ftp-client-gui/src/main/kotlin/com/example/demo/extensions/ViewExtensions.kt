package com.example.demo.extensions

import com.example.demo.FTPClient
import tornadofx.*

fun String.makeLabel(label: String): String {
    return FTPClient.APP_NAME + " - $label"
}
