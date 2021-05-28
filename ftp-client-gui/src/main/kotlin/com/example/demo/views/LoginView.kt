package com.example.demo.views

import com.example.demo.FTPClient
import com.example.demo.extensions.makeLabel
import com.example.demo.viewmodels.LoginViewModel
import com.example.demo.viewmodels.User
import tornadofx.*

class LoginView: View(FTPClient.APP_NAME.makeLabel(this.LABEL)) {
    override val root = Form()

    private val viewmodel = LoginViewModel(User())

    init {
        with(root) {
            paddingAll = 200

            fieldset("Login") {
                field("Username") {
                    textfield(viewmodel.username).required(message = "Insert username")
                }
                field("password") {
                    passwordfield(viewmodel.password).required(message = "Insert password")
                }
            }
        }
    }

    companion object {
        const val LABEL: String = "Connection"
        const val TEXT_FIELD_WIDTH = 200.0
    }
}