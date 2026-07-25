import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "./App";
import { strings } from "./i18n/strings";

test("the root route renders the application name", () => {
  render(<App />);
  const heading = screen.getByRole("heading", { level: 1 });
  expect(heading.textContent).toBe(strings.appName);
});
