import { mount } from 'svelte';
import App from './Root.svelte';
import './style.css';
import './density.css';
import './theme.css';
mount(App, { target: document.getElementById('app')! });
