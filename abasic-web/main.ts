import { default as wasm, JsInterpreter, JsInterpreterState, JsInterpreterOutputType } from "./pkg/abasic_web.js";

wasm().then((module) => {
    const a11yOutputEl = el_with_id('a11y-output');
    const outputEl = el_with_id('output');
    const promptEl = el_with_id('prompt');
    const inputEl = el_with_id('input');
    const formEl = el_with_id('form');

    if (!(inputEl instanceof HTMLInputElement))
        throw new Error("Expected inputEl to be an <input>");

    if (!(formEl instanceof HTMLFormElement))
        throw new Error("Expected formEl to be a <form>");

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

function el_with_id(id: string): HTMLElement {
    const el = document.getElementById(id);
    if (el === null)
        throw new Error(`Element with id "${id}" not found!`);
    return el;
}
