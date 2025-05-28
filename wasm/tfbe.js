import { tfbe_ffi_load_file } from "./bevy_game.js";

window.tfbe_ffi_alert = (message) => {
  alert(message);
};

let instance = null;

window.tfbe_set_instance = (doneInstance) => {
  instance = doneInstance;
};

console.info("create button");
const loadInputButton = document.createElement("input");
loadInputButton.type = "file";
loadInputButton.accept = ".vmf";
loadInputButton.id = "input-file";
loadInputButton.style.position = "absolute";
document.body.appendChild(loadInputButton);

loadInputButton.addEventListener("change", (e) => {
  const file = e.target.files[0];
  if (!file) {
    return;
  }
  const reader = new FileReader();
  reader.onload = (e) => {
    tfbe_ffi_load_file(e.target.result);
  };
  reader.readAsText(file);
});
