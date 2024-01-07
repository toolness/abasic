import { getHTMLElement } from "./util.js";

const a11yOutputEl = getHTMLElement("div", "#a11y-output");
const outputEl = getHTMLElement("div", "#output");
const promptEl = getHTMLElement("label", "#prompt");
const inputEl = getHTMLElement("input", "#input");
const formEl = getHTMLElement("form", "#form");

let latestPartialLine: Text[] = [];

export function print(msg: string) {
  const textNode = document.createTextNode(msg);
  if (msg.endsWith("\n")) {
    latestPartialLine = [];
  } else {
    latestPartialLine.push(textNode);
  }
  outputEl.appendChild(textNode);
  a11yOutputEl.appendChild(textNode.cloneNode());
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
  let prefix = "";
  if (latestPartialLine.length > 0) {
    for (const chunk of latestPartialLine) {
      prefix += chunk.textContent;
      outputEl.removeChild(chunk);
    }
    latestPartialLine = [];
  }
  promptEl.textContent = prefix + prompt;
  a11yOutputEl.appendChild(document.createTextNode(prompt));
}

export function commitCurrentPromptToOutput(additionalText = "") {
  const el = document.createElement("div");

  el.setAttribute("class", "prompt-response");
  el.textContent = `${promptEl.textContent}${additionalText}`;
  outputEl.appendChild(el);
  scroll_output();
}

export function onInputKeyDown(
  callback: (e: KeyboardEvent, el: HTMLInputElement) => void
) {
  inputEl.addEventListener("keydown", (e) => {
    callback(e, inputEl);
  });
}

export function onSubmitInput(callback: () => void) {
  formEl.addEventListener("submit", (e) => {
    e.preventDefault();

    callback();
  });
}

export function getInput(): string {
  return inputEl.value;
}

export function clearInput() {
  inputEl.value = "";
}
