package jp.nonbili.meron.ui

import kotlinx.coroutines.CoroutineDispatcher

/** Dispatcher for blocking core (JNI/FFI) and file work. */
internal expect val ioDispatcher: CoroutineDispatcher
