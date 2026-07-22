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

  it("defaults to a neutral (centered) gaze when no offset is given", () => {
    const markup = renderToStaticMarkup(<BuiltinPet state="idle" emotion="neutral" />);
    expect(markup).toContain('class="builtin-pupils"');
    expect(markup).toContain('transform="translate(0 0)"');
  });

  it("translates the pupils toward the supplied gaze offset", () => {
    const markup = renderToStaticMarkup(
      <BuiltinPet state="idle" emotion="neutral" gaze={{ dx: 3, dy: -2 }} />,
    );
    // Only the pupils and highlights move; the eye sockets stay put so the
    // pupils track within the eyes rather than the whole eye sliding.
    expect(markup).toContain('transform="translate(3 -2)"');
    expect(markup).toContain("builtin-eye-left");
  });
});
