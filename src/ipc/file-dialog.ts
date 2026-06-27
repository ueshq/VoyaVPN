import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

type SaveTextFileOptions = {
  defaultPath: string;
  filters?: Array<{ extensions: string[]; name: string }>;
  text: string;
};

export async function saveTextFile({ defaultPath, filters, text }: SaveTextFileOptions): Promise<string | null> {
  const path = await save({ defaultPath, filters });
  if (!path) {
    return null;
  }

  await writeTextFile(path, text);
  return path;
}
