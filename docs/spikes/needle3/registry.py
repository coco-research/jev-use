"""Our real registry, expressed the way Needle's tool guide says to express it.

One tool per action, named the way a user says it, formats in the description, and a few
deliberately confusable pairs so the test has something to fail on:
    switch_to_app vs open_app, take_note vs search_notes, search_web vs search_notes.
"""
from typing import Literal

import needle

TOOLS = []


# Registers at decoration time. The first cut appended inside a wrapper that was only
# called later, so the list stayed empty and the engine correctly reported "no tools
# declared" - a good reminder that an empty toolset looks like a model failure.
def tool(fn):
    TOOLS.append(fn)
    return fn
@tool
@needle.tool
def switch_to_app(app: str):
    """Bring an app that is already running to the front. Use the app name, e.g. Terminal, Finder, Google Chrome."""
    return {"app": app}


@tool
@needle.tool
def open_app(app: str):
    """Launch an app that is not running yet, e.g. Notes, Preview."""
    return {"app": app}


@tool
@needle.tool
def open_new_tab(browser: str):
    """Open a new tab in a browser, e.g. Google Chrome, Safari."""
    return {"browser": browser}


@tool
@needle.tool
def open_url(url: str):
    """Open a web address in the browser. Give a host or URL, e.g. github.com."""
    return {"url": url}


@tool
@needle.tool
def project_status(project: str):
    """Report the state of one project by name: branch, uncommitted files, last commit."""
    return {"project": project}


@tool
@needle.tool
def uncommitted_work(project: str):
    """List uncommitted changes in a named project."""
    return {"project": project}


@tool
@needle.tool
def what_did_i_work_on(when: str):
    """Summarise what was worked on in a period, e.g. yesterday, this morning, last week."""
    return {"when": when}


@tool
@needle.tool
def list_running_apps():
    """List the apps that are running right now."""
    return {"apps": []}


@tool
@needle.tool
def find_file(name: str):
    """Find a file on this machine by name. Give the file name, e.g. lease.pdf."""
    return {"name": name}


@tool
@needle.tool
def open_path(path: str):
    """Open a folder or file in Finder, e.g. the Downloads folder."""
    return {"path": path}


@tool
@needle.tool
def take_note(text: str):
    """Save a short note for later."""
    return {"text": text}


@tool
@needle.tool
def search_notes(query: str):
    """Search saved notes for a phrase."""
    return {"query": query}


@tool
@needle.tool
def start_timer(minutes: int):
    """Start a countdown timer for a number of minutes."""
    return {"minutes": minutes}


@tool
@needle.tool
def create_event(title: str, when: str):
    """Add an event or reminder with a date or time, e.g. call Sam at 6pm."""
    return {"title": title, "when": when}


@tool
@needle.tool
def send_message(contact: str, body: str):
    """Send a message to a contact by name."""
    return {"contact": contact, "body": body}


@tool
@needle.tool
def draft_email(to: str, subject: str):
    """Draft an email without sending it."""
    return {"to": to, "subject": subject}


@tool
@needle.tool
def search_web(query: str):
    """Search the web for something to read."""
    return {"query": query}


@tool
@needle.tool
def set_volume(level: int):
    """Set the system output volume, 0 to 100."""
    return {"level": level}


@tool
@needle.tool
def lock_screen():
    """Lock the screen."""
    return {"locked": True}


@tool
@needle.tool
def screenshot():
    """Take a screenshot of the screen."""
    return {"taken": True}


@tool
@needle.tool
def toggle_dark_mode(on: bool):
    """Turn dark mode on or off."""
    return {"on": on}


@tool
@needle.tool
def list_capabilities():
    """List what this assistant can do."""
    return {"capabilities": []}
