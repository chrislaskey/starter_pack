<!-- The person's files, newest first, at /files: each with a small
     picture when it is an image, its name, its size, and where it
     stands when it is not on the server yet. A click opens its page. -->
<script>
  import AnimationsToggle from "../lib/AnimationsToggle.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import FileImage from "../lib/FileImage.svelte";
  import { filesContentUrl, filesList, onSynced } from "../lib/core/index.js";
  import { filesNewUrl, filesShowUrl } from "../lib/routes.js";
  import { fileSize } from "./file_size.js";

  let files = $state([]);
  let error = $state("");

  async function load() {
    try {
      files = await filesList();
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<main class="page">
  <AppNav />
  <div class="flex items-center justify-between">
    <h1>Files</h1>
    <a href={filesNewUrl()} class="btn btn-primary btn-sm">New file</a>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if files.length === 0 && !error}
    <p class="muted">No files yet.</p>
  {:else}
    <AnimationsToggle />
  {/if}

  <ul class="files">
    {#each files as { file, shown_as, state, error: problem } (file.id)}
      <li class="file">
        <a href={filesShowUrl(file.id)} class="flex size-12 shrink-0 items-center justify-center">
          {#if shown_as === "image"}
            <FileImage src={filesContentUrl(file.id)} alt={file.name} class="size-12 rounded object-cover" missing="" />
          {/if}
        </a>
        <div class="flex min-w-0 grow flex-col">
          <a href={filesShowUrl(file.id)} class="link truncate">{file.name}</a>
          <span class="muted">{fileSize(file.size)} · {file.inserted_at}</span>
          {#if problem}
            <span class="error text-sm">{problem}</span>
          {/if}
        </div>
        {#if state === "waiting"}
          <span class="badge badge-sm">Waiting to upload</span>
        {:else if state === "refused"}
          <span class="badge badge-warning badge-sm">Not saved on the server</span>
        {/if}
      </li>
    {/each}
  </ul>
</main>
