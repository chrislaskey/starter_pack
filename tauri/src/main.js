import { mount } from "svelte";
import "./app.css";
import StatusPage from "./StatusPage.svelte";

mount(StatusPage, { target: document.getElementById("app") });
