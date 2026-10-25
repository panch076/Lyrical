import { store } from "./store";
import type { GlassStyle, Settings } from "./types";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

type BooleanKey = "showArtwork" | "surroundingLines" | "clickThrough" | "floatingVisible" | "sweep";
const toggles: BooleanKey[] = ["showArtwork", "surroundingLines", "clickThrough", "floatingVisible", "sweep"];

const glassOptions: { value: GlassStyle; label: string }[] = [
  { value: "acrylic", label: "Acrylic" },
  { value: "frosted", label: "Frosted" },
  { value: "clear", label: "Clear" },
  { value: "tinted", label: "Album Tint" },
];

function render(settings: Settings): void {
  for (const key of toggles) $<HTMLInputElement>(key).checked = settings[key];
  $<HTMLInputElement>("fontScale").value = String(settings.fontScale);
  $<HTMLInputElement>("syncOffset").value = String(settings.syncOffset);
  $("syncValue").textContent = `${settings.syncOffset >= 0 ? "+" : ""}${settings.syncOffset.toFixed(2)}s`;
  $<HTMLButtonElement>("syncReset").disabled = settings.syncOffset === 0;
  for (const button of $("glass").querySelectorAll("button")) {
    button.setAttribute("aria-checked", String(button.dataset.value === settings.glass));
  }
}

await store.init();

const glass = $("glass");
for (const option of glassOptions) {
  // Acrylic is a Windows compositor effect.
  if (option.value === "acrylic" && store.platform !== "windows") continue;
  const button = document.createElement("button");
  button.type = "button";
  button.role = "radio";
  button.dataset.value = option.value;
  button.textContent = option.label;
  button.addEventListener("click", () => store.updateSettings({ glass: option.value }));
  glass.append(button);
}

for (const key of toggles) {
  $<HTMLInputElement>(key).addEventListener("change", (event) => {
    const checked = (event.target as HTMLInputElement).checked;
    if (key === "floatingVisible") void store.invoke("set_floating_visible", { visible: checked });
    else store.updateSettings({ [key]: checked });
  });
}

$<HTMLInputElement>("fontScale").addEventListener("change", (e) =>
  store.updateSettings({ fontScale: Number((e.target as HTMLInputElement).value) }),
);
$<HTMLInputElement>("syncOffset").addEventListener("input", (e) =>
  store.updateSettings({ syncOffset: Number((e.target as HTMLInputElement).value) }),
);
$("syncReset").addEventListener("click", () => store.updateSettings({ syncOffset: 0 }));
$("openFullscreen").addEventListener("click", () => void store.invoke("show_fullscreen"));

const autostart = $<HTMLInputElement>("autostart");
autostart.checked = await store.invoke<boolean>("get_autostart");
autostart.addEventListener("change", async () => {
  autostart.checked = await store.invoke<boolean>("set_autostart", { enabled: autostart.checked });
});

$("platformHint").textContent =
  store.platform === "linux"
    ? "Works with any player that supports MPRIS: Spotify, Chrome, Firefox, Rhythmbox and more. On GNOME, if the floating box doesn't stay above other windows, click it, press Alt+Space and choose Always on Top."
    : "Works with any app that shows up in the Windows media controls: Spotify, Chrome or Edge (including YouTube Music), Apple Music, Media Player and more.";

store.on("settings", () => render(store.settings));
render(store.settings);
