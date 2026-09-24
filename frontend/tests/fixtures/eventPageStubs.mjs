import React from "react";

export function useRouter() { return window.fixture.router; }
export default function Element({ children, href, ...props }) {
  return href ? React.createElement("a", { ...props, href }, children) : null;
}
export function loadPageState(key) { return window.fixture.saved[key] ?? null; }
export function savePageState(key, value) { window.fixture.saved[key] = value; }
