package com.example.demo.viewmodels

import javafx.beans.property.SimpleStringProperty
import tornadofx.*

class User {
    var username = SimpleStringProperty()
    var password = SimpleStringProperty()
}

class LoginViewModel(var user: User): ViewModel() {
    var username = bind { user.username }
    var password = bind { user.password }
}