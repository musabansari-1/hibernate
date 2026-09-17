# ContextSleep — Product Hunt / GPT-6 Astra submission copy

Name: ContextSleep
Tagline: Put your workspace to sleep. Wake up ready.
Website: https://contextsleep.pages.dev
Windows beta: https://contextsleep.pages.dev/downloads/contextsleep-0.1.0-windows-private-beta.zip
Pricing: Free beta
Status: Beta / available now
Suggested topics: Productivity, Developer Tools, Windows

Short description:
ContextSleep saves the context that matters across supported apps, safely closes what it can verify, then reconstructs and checks the workspace when you return. The current Windows beta supports VS Code, Chrome, and explicitly approved local development services.

Thumbnail:
contextsleep-thumbnail.png

Gallery order:
1. contextsleep-gallery-1.png — core promise
2. contextsleep-gallery-2.png — Capture → Review → Sleep → Resume
3. contextsleep-gallery-3.png — validation evidence
4. contextsleep-gallery-4.png — GPT-6 Astra engineering story

Maker comment:
Hey Product Hunt 👋

I built ContextSleep because a working session often becomes much bigger than one app: an editor, dozens of browser tabs, local services, folders and terminal context all end up forming one workspace.

Closing everything gets your machine back, but rebuilding that context later is annoying. Keeping everything open preserves context, but keeps consuming resources.

ContextSleep makes that tradeoff explicit. You capture a workspace, review what is actually reconstructable, put supported parts to sleep, then restore them later. The important part is verification: it does not treat “the app launched” as proof that the workspace came back correctly.

The current Windows private beta has a validated lifecycle for VS Code, Chrome and explicitly approved Node/Python services. Explorer folders and verified terminal working directories are restored as context, while arbitrary terminal history, commands and unsupported app state are deliberately not replayed.

GPT-6 Astra helped with some of the hardest engineering work rather than being a chat feature inside the product. One example was VS Code native-window ownership: two workbench windows can share one Code.exe GUI process. After investigating the process/HWND topology, we found no trustworthy stable way to map the exact integration instance to the exact native workbench window. Instead of guessing from titles, focus or timing, ContextSleep intentionally blocks automatic close in that ambiguous case.

This is still a private beta and Windows reboot recovery is not claimed yet. I would especially love feedback on which apps and workspace types ContextSleep should support next.

Astra Challenge — concise explanation:
ContextSleep was built with GPT-6 Astra as an engineering collaborator. Astra helped investigate Windows process/window ownership, durable native-close verification, cold-start recovery, development-service lifecycle design, safety boundaries and end-to-end validation. A key result was proving that multiple VS Code workbenches can share one GUI process without a trustworthy stable integration→HWND mapping; ContextSleep therefore fails closed instead of guessing which native window to close.

Claims to avoid:
- Arbitrary Windows apps are not yet supported.
- Windows reboot recovery is not yet validated.
- Arbitrary terminal commands/history are not restored.
- Do not claim exact/exclusive RAM savings per project.
- The core flow is capture → reviewed close → reconstruction → verification, not literal arbitrary-process suspension.
