import { default as wasm, JsInterpreter, JsInterpreterState, JsInterpreterOutputType } from "./pkg/abasic_web.js";

wasm().then((module) => {
    let interpreter = JsInterpreter.new();
    interpreter.start_evaluating('PRINT "HELLO " 1+2');
    if (interpreter.get_state() === JsInterpreterState.Errored) {
        console.log("ERROR", interpreter.take_latest_error());
        return
    }
    const output = interpreter.take_latest_output();
    for (const item of output) {
        console.log(JsInterpreterOutputType[item.output_type], item.into_string());
    }
});
