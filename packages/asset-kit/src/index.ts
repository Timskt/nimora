import {
  assetPackageSchema,
  type AssetDependency,
  type AssetFile,
  type AssetManifest,
  type AssetPackage,
} from "@nimora/schemas";

export type ImportEntry = Omit<AssetFile, "bytes"> & {
  bytes: number;
};

export type ImportPlan = {
  package: AssetPackage;
  files: readonly AssetFile[];
  dependencies: readonly AssetDependency[];
};

export type ImportFailure = {
  code: "invalid_manifest" | "invalid_inventory";
  message: string;
};

export type ImportResult =
  | { ok: true; plan: ImportPlan }
  | { ok: false; error: ImportFailure };

/**
 * Validates an already isolated file inventory and returns a side-effect-free
 * installation plan. Files are not copied, extracted, or activated here.
 */
export function createImportPlan(
  manifest: AssetManifest,
  entries: readonly ImportEntry[],
  dependencies: readonly AssetDependency[] = [],
): ImportResult {
  const parsed = assetPackageSchema.safeParse({
    manifest,
    files: entries,
    dependencies,
    totalBytes: entries.reduce((total, file) => total + file.bytes, 0),
  });
  if (!parsed.success) {
    return { ok: false, error: { code: "invalid_inventory", message: parsed.error.message } };
  }

  return {
    ok: true,
    plan: {
      package: parsed.data,
      files: parsed.data.files,
      dependencies: parsed.data.dependencies,
    },
  };
}
