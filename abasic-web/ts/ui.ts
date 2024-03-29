import { getHTMLElement } from "./util.js";

const a11yOutputEl = getHTMLElement("div", "#a11y-output");
const outputEl = getHTMLElement("div", "#output");
const promptEl = getHTMLElement("label", "#prompt");
const inputEl = getHTMLElement("input", "#input");
const formEl = getHTMLElement("form", "#form");

let latestPartialLine: Node[] = [];

/**
 * These should all be defined in the CSS.
 */
type SpanClassName = "error" | "error-context" | "warning" | "info";

let temporaryInputHistoryIndex = 0;
let temporaryInputHistory = [""];
let committedInputHistory: string[] = [];

export function printSpanWithClass(msg: string, className: SpanClassName) {
  const span = document.createElement("span");
  span.className = className;
  span.textContent = msg;
  print(span);
}

export function print(msg: string | HTMLSpanElement) {
  let node: Node;
  let text: string;
  if (typeof msg === "string") {
    text = msg;
    node = document.createTextNode(msg);
  } else {
    node = msg;
    text = msg.textContent || "";
  }
  if (text.endsWith("\n")) {
    latestPartialLine = [];
  } else {
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
const IS_IOS =
  /iPad|iPhone|iPod/.test(navigator.userAgent) && !("MSStream" in window);
if (IS_IOS) {
  document.documentElement.classList.add("ugh-ios");
}

function scroll_output() {
  // Different browsers use different elements for scrolling. :(
  [document.documentElement, document.body].forEach((el) => {
    el.scrollTop = el.scrollHeight;
  });
}

export function setPrompt(prompt: string) {
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

const ARROW_UP = "ArrowUp";
const ARROW_DOWN = "ArrowDown";

export function onInputKeyDown(
  callback: (e: KeyboardEvent, el: HTMLInputElement) => void
) {
  inputEl.addEventListener("keydown", (e) => {
    if (e.key === ARROW_UP || e.key === ARROW_DOWN) {
      e.preventDefault();
      let delta = e.key === ARROW_UP ? -1 : 1;
      temporaryInputHistory[temporaryInputHistoryIndex] = inputEl.value;
      let newIndex = temporaryInputHistoryIndex + delta;
      if (newIndex >= 0 && newIndex < temporaryInputHistory.length) {
        temporaryInputHistoryIndex = newIndex;
        inputEl.value = temporaryInputHistory[temporaryInputHistoryIndex];
        let selectionEnd = inputEl.value.length;
        inputEl.setSelectionRange(selectionEnd, selectionEnd);
      }
      return;
    }
    callback(e, inputEl);
  });
}

export function onSubmitInput(callback: () => void) {
  formEl.addEventListener("submit", (e) => {
    e.preventDefault();
    if (inputEl.value !== "") {
      committedInputHistory.push(inputEl.value);
      temporaryInputHistory = [...committedInputHistory, ""];
      temporaryInputHistoryIndex = temporaryInputHistory.length - 1;
    }
    callback();
  });
}

export function getInput(): string {
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
