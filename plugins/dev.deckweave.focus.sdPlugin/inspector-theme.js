// Optional DeckWeave appearance extension; standalone plugins still work in Elgato.
globalThis.DeckWeaveAppearance = {
  active: false,
  subscribe(socket, context) {
    if (!this.active) return;
    socket.addEventListener('open', () => setTimeout(() => {
      if (socket.readyState === 1) socket.send(JSON.stringify({event:'deckweaveSubscribeAppearance', context}));
    }, 0), {once:true});
  },
  apply(appearance) {
    if (!appearance || !['light', 'dark'].includes(appearance.theme)) return;
    this.active = true;
    document.documentElement.dataset.theme = appearance.theme;
    if (/^#[\da-f]{6}$/i.test(appearance.accentColor))
      document.documentElement.style.setProperty('--accent', appearance.accentColor);
  },
  receive(message) {
    if (message.event === 'deckweaveDidChangeAppearance') this.apply(message.payload);
  },
};
