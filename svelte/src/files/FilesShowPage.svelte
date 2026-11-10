<!-- One file, at /files/:id: its bytes shown as the server says its
     type is shown, an image in place, a text or PDF file in a frame,
     anything else by its name and size; with Download, Rename and
     Delete. -->
<script>
  import AnimationsToggle from "../lib/AnimationsToggle.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import FileImage from "../lib/FileImage.svelte";
  import {
    filesContentUrl,
    filesDelete,
    filesDownload,
    filesDownloadSupported,
    filesGet,
    onSynced,
  } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { filesEditUrl, filesIndexUrl } from "../lib/routes.js";
  import { fileSize } from "./file_size.js";

  const id = pathParams().id;
  let shown = $state();
  let canDownload = $state(false);
  let saved = $state("");
  let error = $state("");

  async function load() {
    try {
      [shown, canDownload] = await Promise.all([filesGet(id), filesDownloadSupported()]);
    } catch (problem) {
      error = String(problem);
    }
  }

  async function download() {
    error = "";
    try {
      const path = await filesDownload(id);
      saved = path ? `Saved to ${path}` : "";
    } catch (problem) {
      error = String(problem);
    }
  }

  async function remove() {
    if (!confirm("Delete this file?")) return;
    try {
      await filesDelete(id);
      await navigateToUrl(filesIndexUrl());
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <h1>{shown ? shown.file.name : "File"}</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if shown}
    <p class="muted">
      {shown.file.content_type} · {fileSize(shown.file.size)} · {shown.file.inserted_at}
      {#if shown.state === "waiting"}
        · waiting to upload
      {:else if shown.state === "refused"}
        · not saved on the server
      {/if}
    </p>
    {#if shown.error}
      <p class="error text-sm">{shown.error}</p>
    {/if}

    {#if shown.shown_as === "image"}
      <AnimationsToggle />
      <div>
        <FileImage src={filesContentUrl(id)} alt={shown.file.name} class="file-shown" />
      </div>
    {:else if shown.shown_as === "text"}
      <iframe src={filesContentUrl(id)} title={shown.file.name} class="file-shown" sandbox=""></iframe>
    {:else if shown.shown_as === "pdf"}
      <iframe src={filesContentUrl(id)} title={shown.file.name} class="file-shown"></iframe>
    {:else}
      <p class="muted">This kind of file is not shown here.</p>
    {/if}

    <div class="flex flex-wrap items-center gap-2">
      {#if canDownload}
        <button type="button" class="btn btn-sm" onclick={download}>Download</button>
      {/if}
      <a href={filesEditUrl(id)} class="btn btn-sm">Rename</a>
      <button type="button" class="btn btn-sm btn-ghost" onclick={remove}>Delete</button>
      {#if saved}
        <span class="muted">{saved}</span>
      {/if}
    </div>
  {/if}

  <a href={filesIndexUrl()}>Back to files</a>
</main>
