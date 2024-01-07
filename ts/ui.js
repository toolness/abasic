import { getHTMLElement } from "./util.js";
const a11yOutputEl = getHTMLElement("div", "#a11y-output");
const outputEl = getHTMLElement("div", "#output");
const promptEl = getHTMLElement("label", "#prompt");
const inputEl = getHTMLElement("input", "#input");
const formEl = getHTMLElement("form", "#form");
let latestPartialLine = [];
export function printSpanWithClass(msg, className) {
    const span = document.createElement("span");
    span.className = className;
    span.textContent = msg;
    print(span);
}
export function print(msg) {
    let node;
    let text;
    if (typeof msg === "string") {
        text = msg;
        node = document.createTextNode(msg);
    }
    else {
        node = msg;
        text = msg.textContent || "";
    }
    if (text.endsWith("\n")) {
        latestPartialLine = [];
    }
    else {
        latestPartialLine.push(node);
    }
    outputEl.appendChild(node);
    a11yOutputEl.appendChild(node.cloneNode());
    scroll_output();
}
export function clearScreen() {
    outputEl.textContent = "";
    scroll_output();
}
// See our CSS for .ugh-ios for details on why we're doing this.
const IS_IOS = /iPad|iPhone|iPod/.test(navigator.userAgent) && !("MSStream" in window);
if (IS_IOS) {
    document.documentElement.classList.add("ugh-ios");
}
function scroll_output() {
    // Different browsers use different elements for scrolling. :(
    [document.documentElement, document.body].forEach((el) => {
        el.scrollTop = el.scrollHeight;
    });
}
export function setPrompt(prompt) {
    let prefix = [];
    promptEl.textContent = "";
    if (latestPartialLine.length > 0) {
        for (const chunk of latestPartialLine) {
            promptEl.appendChild(chunk);
        }
        latestPartialLine = [];
    }
    promptEl.appendChild(document.createTextNode(prompt));
    a11yOutputEl.appendChild(promptEl.cloneNode(true));
}
export function commitCurrentPromptToOutput(additionalText = "") {
    const el = document.createElement("div");
    el.setAttribute("class", "prompt-response");
    el.textContent = `${promptEl.textContent}${additionalText}`;
    outputEl.appendChild(el);
    scroll_output();
}
export function onInputKeyDown(callback) {
    inputEl.addEventListener("keydown", (e) => {
        callback(e, inputEl);
    });
}
export function onSubmitInput(callback) {
    formEl.addEventListener("submit", (e) => {
        e.preventDefault();
        callback();
    });
}
export function getInput() {
    return inputEl.value;
}
export function clearInput() {
    inputEl.value = "";
}
export function clearPromptAndDisableInput() {
    setPrompt("");
    clearInput();
    inputEl.disabled = true;
}
