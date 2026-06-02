package com.kwrt.controller

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import com.kwrt.controller.ui.screen.ClientsScreen
import com.kwrt.controller.ui.screen.LoginScreen
import com.kwrt.controller.ui.theme.KwrtTheme
import com.kwrt.controller.vm.AppViewModel
import com.kwrt.controller.vm.Screen

class MainActivity : ComponentActivity() {
    private val vm: AppViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            KwrtTheme {
                val state by vm.state.collectAsState()
                val snackbarHostState = remember { SnackbarHostState() }

                LaunchedEffect(state.snackbar) {
                    state.snackbar?.let {
                        snackbarHostState.showSnackbar(it)
                        vm.dismissSnackbar()
                    }
                }

                Box(
                    Modifier
                        .fillMaxSize()
                        .background(MaterialTheme.colorScheme.background),
                ) {
                    when (state.screen) {
                        Screen.Login -> LoginScreen(state, vm)
                        Screen.Clients -> ClientsScreen(state, vm)
                    }
                    SnackbarHost(
                        snackbarHostState,
                        modifier = Modifier.align(Alignment.BottomCenter),
                    )
                }
            }
        }
    }
}
