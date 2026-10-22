<script lang="ts">
  import { THEMES, savedTheme, setTheme, type Theme } from "./theme";

  // A plain list of the themes. An app that wants pictures builds its own
  // picker on the logic in theme.ts.
  let { onpick }: { onpick?: (theme: Theme) => void } = $props();

  let theme = $state<Theme>(savedTheme());

  function choose(t: Theme) {
    theme = t;
    setTheme(t);
    onpick?.(t);
  }
</script>

<label class="picker">
  <span class="muted">Theme</span>
  <select value={theme} onchange={(e) => choose((e.currentTarget as HTMLSelectElement).value as Theme)}>
    {#each THEMES as [value, label]}<option {value}>{label}</option>{/each}
  </select>
</label>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
</style>
