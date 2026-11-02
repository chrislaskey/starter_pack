<!-- One community and its feed, with each post's thread of comments, as
     App.Web.CommunityLive shows it: any member comments and deletes their
     own comment, a post's owner deletes any comment on it and takes the
     post out of the community. The community is the path's `id`. -->
<script>
  import { invoke } from "@tauri-apps/api/core";
  import Button from "$phoenix/components/Button.svelte";
  import Header from "$phoenix/components/Header.svelte";
  import TextInput from "$phoenix/components/TextInput.svelte";
  import AppNav from "../lib/AppNav.svelte";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { communitiesIndexUrl, postsIndexUrl } from "../lib/routes.js";
  import SyncBar from "../lib/SyncBar.svelte";
  import { submitTo, write } from "../lib/write.js";

  const communityId = pathParams().id;

  // Nothing when the user is not a member, on this device
  let page = $state();
  let loaded = $state(false);
  let error = $state("");

  async function load() {
    try {
      page = await invoke("get_community_page", { communityId });
      loaded = true;
    } catch (problem) {
      error = String(problem);
    }
  }

  const showError = (message) => (error = message);
  const run = async (command, args) => showError(await write(command, args, load));

  async function leave() {
    if (!confirm("Leave this community?")) return;
    error = await write("leave_community", { communityId }, load);
    if (!error) navigateToUrl(communitiesIndexUrl());
  }
</script>

<main class="page">
  <AppNav />
  <Header>
    {page?.community.name ?? "Community"}
    {#snippet subtitle()}
      {#if page}A community with {page.member_count} member(s){/if}
    {/snippet}
    {#snippet actions()}
      {#if page}
        <Button id="leave-community" type="button" onclick={leave}>Leave</Button>
      {/if}
    {/snippet}
  </Header>

  <SyncBar onrefresh={load} />

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if loaded && !page}
    <p class="muted">You are not a member of this community, or this device hasn't synced it yet.</p>
  {/if}

  {#if page}
    <ul id="posts" class="divide-y divide-base-200">
      {#each page.posts as post (post.id)}
        <li id="posts-{post.id}" class="space-y-2 py-3" class:pending={post.pending}>
          <p class="text-sm text-base-content/60">
            {post.owner_email}{#if post.pending}&nbsp;· waiting to sync{/if}
          </p>
          <p class="whitespace-pre-wrap">{post.body}</p>
          {#if post.mine}
            <Button type="button" onclick={() => run("remove_post_from_community", { postId: post.id, communityId })}>
              Remove from {page.community.name}
            </Button>
          {/if}

          <ul id="comments-{post.id}" class="ml-4 space-y-1 border-l border-base-200 pl-3 text-sm">
            {#each post.comments as comment (comment.id)}
              <li id="comment-{comment.id}" class="flex items-start gap-2" class:pending={comment.pending}>
                <span class="text-base-content/60">{comment.owner_email}</span>
                <span class="whitespace-pre-wrap">{comment.body}</span>
                {#if comment.deletable}
                  <Button
                    type="button"
                    class="btn btn-ghost btn-xs"
                    onclick={() => run("delete_comment", { commentId: comment.id })}
                    aria-label="Delete this comment"
                  >
                    ×
                  </Button>
                {/if}
              </li>
            {/each}
          </ul>

          <form
            id="comment-form-{post.id}"
            class="ml-4 flex items-start gap-2"
            onsubmit={submitTo(
              "create_comment",
              (fields) => ({ postsCommunitiesId: post.posts_communities_id, ...fields }),
              load,
              showError,
            )}
          >
            <TextInput
              id="comment-body-{post.id}"
              name="body"
              value=""
              placeholder="Comment"
              aria-label="Comment"
              class="input input-sm w-72"
            />
            <Button type="submit" class="btn btn-sm">Comment</Button>
          </form>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex gap-4">
    <a href={postsIndexUrl()} class="link">Write a post</a>
    <a href={communitiesIndexUrl()} class="link">All communities</a>
  </div>
</main>
