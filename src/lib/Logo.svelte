<script lang="ts">
  let { size = 28 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-label="Ringside">
  <defs>
    <linearGradient id="rs-fire" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffb259" />
      <stop offset="1" stop-color="#ff4d2e" />
    </linearGradient>
    <linearGradient id="rs-ice" x1="1" y1="1" x2="0" y2="0">
      <stop offset="0" stop-color="#2f8cff" />
      <stop offset="1" stop-color="#6fe0ff" />
    </linearGradient>
  </defs>
  <circle cx="32" cy="32" r="27" fill="none" stroke="#1b202b" stroke-width="7" />
  <path d="M32 5 A27 27 0 0 1 56.6 43" fill="none" stroke="url(#rs-fire)" stroke-width="7" stroke-linecap="round" />
  <path d="M51 51.5 A27 27 0 0 1 9.5 17" fill="none" stroke="url(#rs-ice)" stroke-width="7" stroke-linecap="round" />
  <circle cx="32" cy="32" r="15" fill="#141822" stroke="#2a3040" stroke-width="1.5" />
  <path d="M32 23.5 c4.5 4.6 8 7.4 8 10.6 a4 4 0 0 1-6.6 3 l1.3 4.4h-5.4l1.3-4.4a4 4 0 0 1-6.6-3c0-3.2 3.5-6 8-10.6z" fill="#e9edf4" />
</svg>
