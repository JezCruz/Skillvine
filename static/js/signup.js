document.addEventListener("DOMContentLoaded", () => {
  const signupForm = document.getElementById("signupForm");
  const passwordInput = document.getElementById("password");
  const confirmInput = document.getElementById("confirm-password");
  const strengthIndicator = document.getElementById("password-strength");
  const matchIndicator = document.getElementById("password-match");
  const submitBtn = document.getElementById("submit-btn");
  const togglePass = document.getElementById("toggle-password");
  const toggleConfirm = document.getElementById("toggle-confirm");

  // PASSWORD STRENGTH
  passwordInput.addEventListener("input", () => {
    const pwd = passwordInput.value.trim();
    const level = getPasswordStrength(pwd);

    strengthIndicator.className = "password-strength";
    if (!pwd) {
      strengthIndicator.textContent = "";
      return;
    }

    if (level === "weak") {
      strengthIndicator.textContent = "Weak 🔴";
      strengthIndicator.classList.add("weak");
    } else if (level === "medium") {
      strengthIndicator.textContent = "Medium 🟡";
      strengthIndicator.classList.add("medium");
    } else {
      strengthIndicator.textContent = "Strong 🟢";
      strengthIndicator.classList.add("strong");
    }

    checkMatch();
  });

  // CONFIRM PASSWORD MATCH
  confirmInput.addEventListener("input", checkMatch);

  function checkMatch() {
    const password = passwordInput.value.trim();
    const confirm = confirmInput.value.trim();

    if (!confirm) {
      matchIndicator.textContent = "";
      submitBtn.disabled = true;
      return;
    }

    if (password === confirm) {
      matchIndicator.textContent = "Passwords match ✅";
      matchIndicator.className = "password-match success";
      submitBtn.disabled = false;
    } else {
      matchIndicator.textContent = "Passwords do not match ❌";
      matchIndicator.className = "password-match error";
      submitBtn.disabled = true;
    }
  }

  // SHOW/HIDE PASSWORD
  togglePass.addEventListener("click", () => toggleVisibility(passwordInput, togglePass));
  toggleConfirm.addEventListener("click", () => toggleVisibility(confirmInput, toggleConfirm));

  function toggleVisibility(input, toggle) {
    const isHidden = input.type === "password";
    input.type = isHidden ? "text" : "password";
    toggle.textContent = isHidden ? "🙈" : "👁";
  }

  // PASSWORD STRENGTH LOGIC
  function getPasswordStrength(password) {
    let score = 0;
    if (password.length >= 8) score++;
    if (/[A-Z]/.test(password)) score++;
    if (/[0-9]/.test(password)) score++;
    if (/[^A-Za-z0-9]/.test(password)) score++;

    if (score <= 1) return "weak";
    if (score <= 3) return "medium";
    return "strong";
  }

  // SIGNUP FORM SUBMIT
  signupForm.addEventListener("submit", async (e) => {
    e.preventDefault();

    const full_name = signupForm.full_name.value.trim();
    const email = signupForm.email.value.trim();
    const password = signupForm.password.value.trim();
    const confirm = signupForm.confirm_password.value.trim();
    const role = signupForm.role.value;

    if (!full_name || !email || !password || !confirm || !role) {
      return showError("Please fill all fields.");
    }
    if (password !== confirm) {
      return showError("Passwords do not match.");
    }

    try {
      const response = await fetch("/signup", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ full_name, email, password, role }),
        credentials: "include",
      });

      const text = await response.text();

      if (response.ok) {
        // Try parsing JSON safely
        let data;
        try {
          data = JSON.parse(text);
        } catch {
          data = { redirect: "/login" }; // fallback
        }

        // Show success notification then redirect (auto-login should have set session cookie)
        showNotice("Registered successfully", "success");
        setTimeout(() => {
          window.location.href = data.redirect;
        }, 1500);
      } else {
        showError(text || "Signup failed. full_name or email may already exist.");
      }
    } catch (err) {
      console.error(err);
      showError("Network error. Please try again.");
    }
  });

  function showError(msg) {
    alert(msg); // replace with fancy UI if needed
  }
  
  function showNotice(message, type = "info", duration = 2000) {
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
