package com.example.demo.models

import javafx.beans.property.ListProperty
import javafx.beans.property.SimpleListProperty
import javafx.scene.Parent
import tornadofx.*

class Folder(val name: String) {

    val folders: ListProperty<Folder> = SimpleListProperty(
        mutableListOf<Folder>().observable()
    )

    val files: ListProperty<String> = SimpleListProperty(
        mutableListOf<String>().observable()
    )

}
