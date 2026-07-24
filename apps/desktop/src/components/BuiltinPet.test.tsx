import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BuiltinPet } from "./BuiltinPet";

describe("BuiltinPet", () => {
  it("renders a transparent vector character with expressive anatomy", () => {
    const markup = renderToStaticMarkup(<BuiltinPet state="walking" emotion="happy" mood={72} animation="pet.walk" />);
    expect(markup).toContain("builtin-pet overlay-pet walking emotion-happy");
    expect(markup).toContain("builtin-tail");
    expect(markup).toContain("builtin-ear-left");
    expect(markup).toContain("builtin-eye-left");
    expect(markup).toContain("builtin-paws");
    expect(markup).toContain("builtin-contact-shadow");
    expect(markup).toContain('data-state="walking"');
    expect(markup).toContain('data-emotion="happy"');
    expect(markup).toContain('data-mood="72"');
    expect(markup).toContain('data-mood-band="high"');
    expect(markup).toContain('data-animation="pet.walk"');
    expect(markup).not.toContain("<div");
  });
});
