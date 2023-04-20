use std::ops::Deref;
use jni::objects::{JClass, JString};
use jni::JNIEnv;
use jni::sys::{jint, jlong, jstring};
use parser::tokens::tokenizer::{ParserState, Tokenizer};

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_start
(mut environment: JNIEnv, _class: JClass, buffer: jstring, _start: jint, _end: jint, _state: jint) -> jlong {
    let string = unsafe { JString::from_raw(buffer) };
    let string = environment.get_string(&string).unwrap();
    let string = string.to_str().unwrap().as_bytes();

    let tokenizer = Tokenizer::new(string);
    return Box::into_raw(Box::new(tokenizer)) as jlong;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getState
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jint {
    return Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).state.last().unwrap().clone() as jint;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getTokenType
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jint {
    return Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).last.token_type.clone() as jint;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getTokenStart
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jint {
    return Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).last.start as jint;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getTokenEnd
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jint {
    return Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).last.end as jint;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_advance
(_environment: JNIEnv, _class: JClass, reference: jlong) {
    Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).next();
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getCurrentPosition
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jlong {
    return Box::into_raw(Box::new(Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).serialize())) as jlong;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_restore
(_environment: JNIEnv, _class: JClass, reference: jlong, state: jlong) {
    Box::leak(unsafe { Box::from_raw(reference as *mut Tokenizer) }).load(
        Box::leak(unsafe { Box::from_raw(state as *mut ParserState) }));
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getPositionOffset
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jint {
    return Box::leak(unsafe { Box::from_raw(reference as *mut ParserState) }).index as jint;
}

pub fn Java_bigbade_raven_ravenintellijplugin_natives_NativeParserRunner_getPositionState
(_environment: JNIEnv, _class: JClass, reference: jlong) -> jint {
    return Box::leak(unsafe { Box::from_raw(reference as *mut ParserState) }).state.last().unwrap().clone() as jint;
}