/**
 * Find an element with the given HTML tag and selector, raising an exception
 * if it's not found.
 *
 * @param tagName The name of the element's HTML tag.
 * @param selector The selector for the element, not including its HTML tag.
 * @param parent The parent node to search within (defaults to `document`).
 *
 * This function was taken from: https://github.com/JustFixNYC/justfix-ts/blob/master/packages/util/get-html-element.ts
 */
export function getHTMLElement<K extends keyof HTMLElementTagNameMap>(
  tagName: K,
  selector: string,
  parent: ParentNode = document
): HTMLElementTagNameMap[K] {
  const finalSelector = `${tagName}${selector}`;
  const node = parent.querySelector(finalSelector);
  if (!node) {
    throw new Error(`Couldn't find any elements matching "${finalSelector}"`);
  }
  return node as HTMLElementTagNameMap[K];
}

export function unreachable(arg: never) {
  throw new Error(`Assertion failure, unreachable(${arg}) was called!`);
}
