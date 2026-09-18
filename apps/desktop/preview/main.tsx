// DEV ONLY design harness entry. Not part of the production bundle.
import React from 'react';
import {createRoot} from 'react-dom/client';
import '../src/style.css';
import {installMockBridge} from './mock';
installMockBridge();
const {App} = await import('../src/App');
createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
