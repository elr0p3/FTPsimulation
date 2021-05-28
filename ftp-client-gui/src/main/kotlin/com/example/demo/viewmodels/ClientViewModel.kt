package com.example.demo.viewmodels

import javafx.beans.property.SimpleListProperty
import javafx.beans.property.SimpleStringProperty
import sun.jvm.hotspot.runtime.Bytes
import tornadofx.ViewModel
import tornadofx.observable

class ClientViewModel: ViewModel() {

    enum class FTPCommand(val value: String) {
        CD("CD"),
        DELETE("DELETE"),
        GET("GET"),         // bytes
        LS("LS"),           // bytes
        MKDIR("MKDIR"),
        PUT("PUT"),         // bytes
        PWD("PWD"),
        QUIT("QUIT"),
        RMDIR("RMDIR"),
        MV("MV")
    }

    val commandList = SimpleListProperty(FTPCommand.values().toList().map { it.value }.sorted().observable())
    var selectedCommand = SimpleStringProperty("None")
    var previousCommand = SimpleStringProperty("None")
    var arg = SimpleStringProperty("")
    var results = SimpleStringProperty("No results yet")

    fun run() {
        val command = buildCommand()
        // TODO: call the request method here and catch he results
        val requestResult = command // this will wither receive bytes or a string
        val willReceiveBytes =    selectedCommand.value == FTPCommand.GET.value ||
                selectedCommand.value == FTPCommand.PUT.value ||
                selectedCommand.value == FTPCommand.LS.value
        var result: String = ""
        result = if (willReceiveBytes) {
            interpretBytes(requestResult.toByteArray()) //TODO: change this so this doesnt convert to ByteArray
        } else {
            requestResult
        }

        results.value = result
    }

    private fun buildCommand(): String = "${selectedCommand.value} ${arg.value}"

    private fun interpretBytes(bytes: ByteArray): String = bytes.toString()
}