package com.example.demo.views

import com.example.demo.FTPClient
import com.example.demo.extensions.makeLabel
import com.example.demo.viewmodels.ConnectionViewModel
import com.example.demo.viewmodels.SocketAddress
import javafx.geometry.Orientation
import javafx.scene.control.Alert
import javafx.scene.control.Button
import javafx.scene.control.ProgressIndicator
import tornadofx.*

class ConnectionView: View(FTPClient.APP_NAME.makeLabel(this.LABEL)) {

    override val root = Form()

    private val viewmodel = ConnectionViewModel(SocketAddress())

    init {
    //override val root = hbox (alignment = Pos.CENTER) {
        with (root) {
            paddingAll = 200
            fieldset("Connection") {
                labelPosition = Orientation.VERTICAL

                field("IP address") {
                    textfield(viewmodel.ip).required(message = "Insert IP address")
                }

                field("Port") {
                    textfield(viewmodel.port).required(message = "Insert port")
                }
            }

            button("Connect") {
                setOnAction {
                    //TODO: Establish connection with the Server
                    connect()
                }
            }
        }
    }

    private fun Button.connect() {
        if (viewmodel.validIP() && viewmodel.validPort()) {
            var graphic = ProgressIndicator()

            runAsync {
                viewmodel.connect()
            } ui { success ->
                //graphic = null
                if (success) {
                    replaceWith(ClientView::class, ViewTransition.FadeThrough(1.seconds))
                } else {
                    alert(Alert.AlertType.WARNING, "Connection error", "Check whether IP or Port is correct")
                }
            }
        }
    }

    companion object {
        const val LABEL: String = "Connection"
        const val TEXT_FIELD_WIDTH = 200.0
    }
}

