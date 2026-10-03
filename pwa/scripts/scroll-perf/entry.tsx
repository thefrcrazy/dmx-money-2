import React from 'react';
import { createRoot } from 'react-dom/client';
import { FixtureProvider } from './bank';
import Transactions from '../../src/pages/Transactions';
import { ToastProvider } from '../../src/context/ToastContext';
import './style.css';
createRoot(document.getElementById('root')!).render(<FixtureProvider><ToastProvider><main style={{height:'100dvh', overflowY:'auto', padding:16}}><Transactions /></main></ToastProvider></FixtureProvider>);
