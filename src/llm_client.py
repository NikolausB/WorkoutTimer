import json
import urllib.request
import urllib.error


DEFAULT_SYSTEM_PROMPT = """You are a professional fitness coach. Create training plans based on the user's request.

You MUST respond ONLY with valid JSON in this exact format, no other text, no markdown:
{
  "name": "Plan Name",
  "total_rounds": 1,
  "rest_between_rounds_seconds": 60,
  "exercises": [
    {"name": "Exercise Name", "duration_seconds": 30, "reps": null, "rest_seconds": 30},
    {"name": "Exercise Name", "duration_seconds": null, "reps": 10, "rest_seconds": 60}
  ]
}

Rules:
- For timed exercises, set duration_seconds (>= 10) and reps to null
- For rep-based exercises, set reps (>= 1) and duration_seconds to null
- rest_seconds is always required (0-300, use 0 if no rest needed)
- total_rounds is how many times to repeat ALL exercises (1 = single pass, 3 = repeat 3 times). Use 1 unless the user asks for multiple rounds/circuits.
- rest_between_rounds_seconds is the pause in seconds between rounds when total_rounds > 1 (0-600, default 60). Use 0 if no rest between rounds is desired.
- Include 4-10 exercises per plan
- Use common exercise names (Pushups, Sit-Up, Bodyweight Squat, Plank, Pullups, Dips, Lunges, Crunches, Burpees, Mountain Climbers, Barbell Squat, Barbell Bench Press, Barbell Deadlift, Kettlebell Swings, Rope Jumping, etc.)
- Respond with ONLY the JSON object, no markdown, no explanation"""


def chat_completion(
    base_url: str,
    api_key: str,
    model: str,
    messages: list[dict],
    timeout: int = 120,
) -> str:
    """Send a chat completion request to an OpenAI-compatible API.

    Args:
        base_url: The API base URL (e.g., "http://localhost:11434/v1")
        api_key: The API key (empty string for local Ollama)
        model: The model name (e.g., "llama3.2")
        messages: List of message dicts with "role" and "content"
        timeout: Request timeout in seconds

    Returns:
        The assistant's response text.

    Raises:
        LLMError: On connection, HTTP, or parsing errors.
    """
    url = base_url.rstrip("/") + "/chat/completions"
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"

    body = json.dumps({
        "model": model,
        "messages": messages,
        "temperature": 0.7,
        "stream": False,
    })

    req = urllib.request.Request(url, data=body.encode("utf-8"), headers=headers, method="POST")

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            return data["choices"][0]["message"]["content"]
    except urllib.error.HTTPError as e:
        body_text = ""
        try:
            body_text = e.read().decode("utf-8", errors="replace")
        except Exception:
            pass
        if e.code == 401 or e.code == 403:
            raise LLMError(f"Invalid API key. Check your provider settings.\n{body_text}")
        if e.code == 404:
            raise LLMError(f"Model '{model}' not found. Check your model name.\n{body_text}")
        raise LLMError(f"HTTP error {e.code}: {body_text}")
    except urllib.error.URLError as e:
        raise LLMError(f"Cannot connect to {base_url}. Make sure it's running.\n{e.reason}")
    except json.JSONDecodeError:
        raise LLMError("AI returned invalid response format.")
    except KeyError:
        raise LLMError("AI returned unexpected response structure.")
    except Exception as e:
        raise LLMError(f"Request failed: {e}")


class LLMError(Exception):
    pass


def build_history_context(sessions: list) -> str:
    """Build a concise training history summary for the LLM prompt."""
    if not sessions:
        return "No training history yet."

    from collections import Counter
    plan_counts = Counter(s.plan_name for s in sessions[:50])
    lines = ["Recent training history:"]
    for name, count in plan_counts.most_common(10):
        last = next((s for s in sessions if s.plan_name == name), None)
        last_date = last.started_at.strftime("%Y-%m-%d") if last else "unknown"
        lines.append(f"- {name}: {count} session{'s' if count != 1 else ''}, last {last_date}")
    return "\n".join(lines)


def parse_plan_response(response_text: str) -> dict | None:
    """Try to parse the LLM response as a training plan JSON.

    Handles responses that may contain markdown code blocks.
    Returns the parsed dict or None if parsing fails.
    """
    text = response_text.strip()
    if text.startswith("```"):
        lines = text.split("\n")
        lines = [l for l in lines if not l.startswith("```")]
        text = "\n".join(lines).strip()
    if text.startswith("```json"):
        lines = text.split("\n")
        lines = [l for l in lines if not l.startswith("```")]
        text = "\n".join(lines).strip()

    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        start = text.find("{")
        end = text.rfind("}")
        if start >= 0 and end > start:
            try:
                data = json.loads(text[start:end + 1])
            except json.JSONDecodeError:
                return None
        else:
            return None

    if "name" not in data or "exercises" not in data:
        return None
    if not isinstance(data["exercises"], list):
        return None

    return data