import { mount } from "svelte";
import "./app.css";
import TmuxExamplePage from "./TmuxExamplePage.svelte";

mount(TmuxExamplePage, { target: document.getElementById("app") });
