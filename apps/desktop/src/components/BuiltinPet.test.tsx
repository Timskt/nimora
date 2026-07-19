import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BuiltinPet } from "./BuiltinPet";

describe("BuiltinPet", () => {
  it("renders a transparent vector character with expressive anatomy", () => {
    const markup = renderToStaticMarkup(<BuiltinPet state="walking" emotion="happy" />);
    expect(markup).toContain("builtin-pet overlay-pet walking emotion-happy");
    expect(markup).toContain("builtin-tail");
    expect(markup).toContain("builtin-ear-left");
    expect(markup).toContain("builtin-eye-left");
    expect(markup).toContain("builtin-paws");
    expect(markup).not.toContain("<div");
  });
});
