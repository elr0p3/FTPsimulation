package com.example.demo.views

import com.example.demo.FTPClient
import com.example.demo.extensions.makeLabel
import com.example.demo.viewmodels.ClientViewModel
import javafx.geometry.Pos
import javafx.scene.control.Button
import javafx.scene.layout.Border
import javafx.scene.layout.BorderStroke
import javafx.scene.layout.StackPane
import javafx.scene.layout.VBox
import tornadofx.*

class ClientView: View(FTPClient.APP_NAME.makeLabel(this.LABEL)) {

    override val root = VBox()

    private val viewmodel: ClientViewModel by inject()

    init {
        with(root) {
            alignment = Pos.CENTER

            text(viewmodel.results) {

            }

            hbox(4) {
                alignment = Pos.CENTER

                combobox(viewmodel.selectedCommand, viewmodel.commandList) {

                }

                textfield(viewmodel.arg) {
                    prefWidth = 300.0
                    minWidth = 300.0
                }

                button("Run") {
                    setOnAction {
                        //TODO: Send the command via the Client to the server
                        runCommand()
                    }
                }
            }
        }
    }

    fun Button.runCommand() {
        if (viewmodel.previousCommand != viewmodel.selectedCommand) {
            runAsync {
                viewmodel.run()
            }
        }
    }

    /*override val root = vbox(alignment = Pos.TOP_LEFT) {
        drawer {
            prefWidth = 200.0
            maxWidth = 300.0

            item("File Tree", expanded = true) {
                treeview<Any>(root = TreeItem("C:")) {
                    cellFormat {
                        text = when (it) {
                            is String -> it
                            else -> kotlin.error("Invalid value type")
                        }
                    }
                    populate { parent ->
                        val value = parent.value
                        when (parent) {
                            root -> vm.fileTree
                            else -> null
                        }
                    }
                }
            }
        }
    }*/

    companion object {
        const val LABEL: String = "Client"
    }
}