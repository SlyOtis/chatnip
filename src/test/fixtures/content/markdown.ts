export const allMarkdown = [
  '**Agenda** for *fredag* and _torsdag_, ~onsdag~ er avlyst',
  '> Husk å oppdatere `Cargo.lock` først',
  '- gå gjennom PR',
  '- bestem leverandør',
  '1. demo',
  '2. retro',
  '```rust',
  'fn main() {',
  '    println!("<hei>");',
  '}',
  '```',
  'Notater: https://northwind.example/agenda.',
  'Spør @Eva om skissene'
].join('\n')

export const fencedWithoutLanguage = '```\nnpm run dev && echo "ok"\n```'

export const snakeCase = 'rename some_variable_name and keep _real emphasis_ here'

export const mentionAtEnd = 'takk @Sam'

export const emailAddress = 'skriv til anna.berg@client.example'

export const urlInCode = 'kjør `curl https://api.northwind.example/health`'

export const longMessage = Array.from({ length: 200 }, (_, i) => `${String(i + 1).padStart(3, '0')} Logglinje fra sync-løkka med æøå og 🚀, se https://northwind.example/logg/${i + 1} for detaljer om kjøringen.`).join('\n')

export const rtlMixed = 'مرحبا Milo! שלום 👋 Привет, møtet er kl 10 ✅'

export const documentMarkdown = ['# Sprintplan uke 43', '## Mål', '- [x] Mock server med rundtur', '- [ ] Skrivevarsel', '', '## Fordeling', '| Oppgave | Eier | Status |', '|---|---|---|', '| Seed | Milo | Ferdig |', '| Statistikk | Eva | Pågår |', '', '---', 'Ferdig'].join('\n')
