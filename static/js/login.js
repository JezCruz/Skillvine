document.addEventListener("DOMContentLoaded", () => {
  const loginForm = document.getElementById("login-form");
  const emailInput = document.getElementById("login-email");
  const passwordInput = document.getElementById("login-password");
  const submitBtn = loginForm.querySelector('button[type="submit"]');
  const errorBox = document.getElementById("error-box");
  const togglePass = document.getElementById("toggle-login-password");

  togglePass.addEventListener("click", (e) => {
    e.preventDefault();
    const isHidden = passwordInput.type === "password";
    passwordInput.type = isHidden ? "text" : "password";
    togglePass.textContent = isHidden ? "🙈" : "👁";
  });

  loginForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    errorBox.textContent = "";

    const email = emailInput.value.trim();
    const password = passwordInput.value.trim();

    if (!email || !password) {
      return showError("Please enter both email and password.");
    }

    submitBtn.disabled = true;
    submitBtn.textContent = "Logging in...";

    try {
      const formData = new URLSearchParams();
      formData.append("email", email);
      formData.append("password", password);

      const response = await fetch("/login", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: formData.toString(),
        credentials: "include",
      });

      if (!response.ok) {
        // Handle 401 JSON errors
        const contentType = response.headers.get("Content-Type") || "";
        if (contentType.includes("application/json")) {
          const errData = await response.json();
          return showError(errData.error || "Invalid email or password");
        } else {
          const text = await response.text();
          return showError(text || "Invalid email or password");
        }
      }

      // Success — determine redirect target then show welcome notice
      const contentType = response.headers.get("Content-Type") || "";
      let redirect = "/";
      if (contentType.includes("application/json")) {
        // JSON response may include redirect
        try {
          const data = await response.json();
          redirect = data.redirect || "/";
        } catch (e) {
          // ignore JSON parse error
        }
      } else if (response.redirected) {
        // fetch followed a redirect; use final URL
        redirect = response.url || "/";
      } else {
        // fallback to Location header
        redirect = response.headers.get("location") || "/";
      }

      showNotice("Welcome back!", "success", 3000);
      setTimeout(() => {
        window.location.href = redirect;
      }, 3000);
    } catch (err) {
      console.error(err);
      showError("Network or server error. Please try again.");
    } finally {
      submitBtn.disabled = false;
      submitBtn.textContent = "Login";
    }
  });

  function showError(message) {
    errorBox.textContent = message;
    errorBox.classList.add("show");
    setTimeout(() => errorBox.classList.remove("show"), 4000);
  }

  function showNotice(message, type = "info", duration = 3000) {
    let el = document.getElementById("top-notice");
    if (!el) {
      el = document.createElement("div");
      el.id = "top-notice";
      el.className = "top-notice";
      document.body.appendChild(el);
    }
    el.textContent = message;
    el.classList.add("show");
    el.classList.remove("hide");
    if (duration > 0) {
      setTimeout(() => {
        el.classList.remove("show");
        el.classList.add("hide");
      }, duration);
    }
  }
});
