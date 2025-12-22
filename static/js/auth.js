document.addEventListener("DOMContentLoaded", () => {
  const signupForm = document.getElementById("signup-form");
  if (!signupForm) return; // Stop script if form does not exist (prevents null errors)

  const full_nameInput = document.getElementById("full_name");
  const emailInput = document.getElementById("email");
  const passwordInput = document.getElementById("password");
  const confirmInput = document.getElementById("confirm-password");
  const roleSelect = document.getElementById("role");
  const submitBtn = document.getElementById("submit-btn");
  const strengthIndicator = document.getElementById("password-strength");
  const matchIndicator = document.getElementById("password-match");
  const togglePass = document.getElementById("toggle-password");
  const toggleConfirm = document.getElementById("toggle-confirm");

  // Create error box in case not already present
  let errorBox = document.getElementById("error-box");
  if (!errorBox) {
    errorBox = document.createElement("div");
    errorBox.id = "error-box";
    errorBox.className = "error-box";
    signupForm.prepend(errorBox);
  }

  // Password strength
  passwordInput.addEventListener("input", () => {
    const pwd = passwordInput.value.trim();
    const level = getPasswordStrength(pwd);

    strengthIndicator.className = "password-strength";
    if (!pwd) { strengthIndicator.textContent = ""; return; }

    strengthIndicator.textContent = level === "weak" ? "Weak 🔴"
      : level === "medium" ? "Medium 🟡"
      : "Strong 🟢";

    strengthIndicator.classList.add(level);
    checkMatch();
  });

  // Password match
  confirmInput.addEventListener("input", checkMatch);

  function checkMatch() {
    const password = passwordInput.value;
    const confirm = confirmInput.value;

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

  // Toggle password visibility
  function toggleVisibility(input, toggle) {
    if (!input || !toggle) return;
    const isHidden = input.type === "password";
    input.type = isHidden ? "text" : "password";
    toggle.textContent = isHidden ? "🙈" : "👁";
  }

  if (togglePass) togglePass.addEventListener("click", () => toggleVisibility(passwordInput, togglePass));
  if (toggleConfirm) toggleConfirm.addEventListener("click", () => toggleVisibility(confirmInput, toggleConfirm));

  // Strength calculator
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

  // Signup + Auto-login
  signupForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    errorBox.textContent = "";

    const full_name = full_nameInput.value.trim();
    const email = emailInput.value.trim();
    const password = passwordInput.value.trim();
    const confirm = confirmInput.value.trim();
    const role = roleSelect.value;

    if (!full_name || !email || !password || !confirm || !role)
      return showError("Please fill all fields.");

    if (password !== confirm)
      return showError("Passwords do not match.");

    try {
      // 1. Send signup request
      const signupRes = await fetch("/signup", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ full_name, email, password, role }),
        credentials: "include",
      });

      const signupData = await signupRes.json();
      if (!signupRes.ok) return showError(signupData.error || "Signup failed.");

      // 2. Auto-login
      const loginRes = await fetch("/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ full_name, password }),
        credentials: "include",
      });

      if (!loginRes.ok) return showError("Registered, but auto-login failed. Login manually.");

      // 3. Show welcome message + redirect
      document.body.innerHTML = `
        <div class="welcome-message">
          🎉 Welcome to Skillvine, <strong>${full_name}</strong>!<br>
          Role: <strong>${role}</strong><br>
          Redirecting...
        </div>
      `;
      setTimeout(() => window.location.href = signupData.redirect || "/dashboard", 1800);

    } catch (err) {
      console.error(err);
      showError("Network or server error. Try again.");
    }
  });

  function showError(message) {
    errorBox.textContent = message;
    errorBox.classList.add("show");
    setTimeout(() => errorBox.classList.remove("show"), 4000);
  }
});
