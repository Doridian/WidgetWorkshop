// The browser build's storage and start form (docs/web.md), for
// src/web.rs. Everything is kept in one IndexedDB database:
//
// - "cd": the game's CD image the player picked, as the Blob the file
//   input gave (key "image": { blob, name, size }). IndexedDB keeps
//   Blobs without reading them into memory, in every browser and on the
//   page's own thread.
// - "files": the port's own files, the widgets the player saves
//   (*.WGT), WIDGET.INI and settings.ini, by their path below the
//   storage's mount point, as Uint8Arrays.
//
// Nothing is fetched or sent anywhere: the files come from the player.

const DB = "widget-workshop";
let opened = null;

function db() {
  if (!opened) {
    opened = new Promise((resolve, reject) => {
      const req = indexedDB.open(DB, 1);
      req.onupgradeneeded = () => {
        req.result.createObjectStore("cd");
        req.result.createObjectStore("files");
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }
  return opened;
}

function request(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function store(name, mode) {
  return (await db()).transaction(name, mode).objectStore(name);
}

function done(tx) {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

// The changes to "files", one after the other in the order the game
// made them.
let queue = Promise.resolve();

function change(f) {
  queue = queue
    .then(async () => {
      const s = await store("files", "readwrite");
      f(s);
      await done(s.transaction);
    })
    .catch((e) => console.error("storage:", e));
}

export function putFile(path, data) {
  // `data` is a view of the module's memory: copy it now.
  const copy = new Uint8Array(data);
  change((s) => s.put(copy, path));
}

export function deleteFile(path) {
  change((s) => s.delete(path));
}

export async function storedFiles() {
  await queue;
  const s = await store("files", "readonly");
  const [keys, values] = await Promise.all([request(s.getAllKeys()), request(s.getAll())]);
  return keys.map((k, i) => [String(k), new Uint8Array(values[i])]);
}

export async function readBlob(blob, start, end) {
  return new Uint8Array(await blob.slice(start, end).arrayBuffer());
}

export async function forgetCd() {
  const s = await store("cd", "readwrite");
  s.delete("image");
  await done(s.transaction);
}

function size(n) {
  return (n / (1024 * 1024)).toFixed(0) + " MB";
}

// Shows the start form (#setup) and resolves to the CD image once the
// player presses Start with one, the pressing being the user gesture the
// browser wants before the game's sound may start. A newly picked image
// is stored first. `error` says why the last one didn't
// do.
export async function chooseCd(error) {
  const $ = (id) => document.getElementById(id);
  let stored = null;
  try {
    stored = await request((await store("cd", "readonly")).get("image"));
  } catch (e) {
    console.error("storage:", e);
  }
  $("setup").hidden = false;
  $("setup-error").textContent = error || "";
  $("setup-error").hidden = !error;
  $("cd-stored").hidden = !stored;
  if (stored) $("cd-name").textContent = `${stored.name} (${size(stored.size)})`;
  const cd = $("cd-file");
  cd.value = "";
  const update = () => ($("start").disabled = !(stored || cd.files.length));
  cd.onchange = update;
  update();
  $("start").focus();
  return new Promise((resolve) => {
    $("start").onclick = async () => {
      $("start").disabled = true;
      let blob = stored && stored.blob;
      const file = cd.files[0];
      const note = [];
      if (file) {
        blob = file;
        try {
          const s = await store("cd", "readwrite");
          s.put({ blob: file, name: file.name, size: file.size }, "image");
          await done(s.transaction);
        } catch (e) {
          console.error("storage:", e);
          note.push("The browser didn't keep the CD image; it asks again next time.");
        }
      }
      // Asks the browser not to clear the storage when space runs low.
      if (navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => {});
      for (const n of note) console.warn(n);
      $("setup").hidden = true;
      resolve(blob);
    };
  });
}
