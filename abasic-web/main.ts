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

    function print(msg: string) {
        const textNode = document.createTextNode(msg);
        outputEl.appendChild(textNode);
        a11yOutputEl.appendChild(textNode.cloneNode());
        scroll_output();
    }

    function clearScreen() {
        outputEl.textContent = "";
        scroll_output();
    }

    // See our CSS for .ugh-ios for details on why we're doing this.
    const IS_IOS = /iPad|iPhone|iPod/.test(navigator.userAgent) && !('MSStream' in window);
    if (IS_IOS) {
        document.documentElement.classList.add('ugh-ios');
    }

    clearScreen();

    let interpreter = JsInterpreter.new();
    interpreter.start_evaluating('PRINT "HELLO " 1+2');
    if (interpreter.get_state() === JsInterpreterState.Errored) {
        print(interpreter.take_latest_error() ?? "");
        return
    }
    const output = interpreter.take_latest_output();
    for (const item of output) {
        if (item.output_type  === JsInterpreterOutputType.Print) {
            print(item.into_string());
        } else {
            // TODO: Print this in a different color?
            print(`${item.into_string()}\n`);
        }
    }
});

function el_with_id(id: string): HTMLElement {
    const el = document.getElementById(id);
    if (el === null)
        throw new Error(`Element with id "${id}" not found!`);
    return el;
}

function scroll_output() {
    // Different browsers use different elements for scrolling. :(
    [document.documentElement, document.body].forEach(el => {
        el.scrollTop = el.scrollHeight;
    });
}
