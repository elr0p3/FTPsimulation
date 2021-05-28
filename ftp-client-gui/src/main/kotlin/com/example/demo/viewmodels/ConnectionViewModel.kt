package com.example.demo.viewmodels

import javafx.beans.property.SimpleBooleanProperty
import javafx.beans.property.SimpleIntegerProperty
import javafx.beans.property.SimpleStringProperty
import tornadofx.ViewModel

class SocketAddress {
    var ip  = SimpleStringProperty()
    var port  = SimpleStringProperty()
}

class ConnectionViewModel(var socketAddress: SocketAddress): ViewModel() {
    var ip = bind { socketAddress.ip }
    var port = bind { socketAddress.port }

    fun validIP(): Boolean {
        val inputIP = ip.value ?: return false
        val splitIP = inputIP.split(".").map { it.toInt() }
        if (splitIP.size != 4) return false
        for (n in splitIP) {
            if (n !in 0..255) return false
        }
        return true
    }

    fun validPort(): Boolean {
        val inputPort = port.value ?: return false
        if (inputPort.toInt() < 0) return false
        return true
    }

    fun connect(): Boolean {
        // TODO: here we will connect with the server with the given parameters
        return true
    }
}