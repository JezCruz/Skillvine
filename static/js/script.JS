document.addEventListener("DOMContentLoaded", () => {
  /* ---------------------------------------------------
     🌿 ELEMENT REFERENCES
  --------------------------------------------------- */
  const heroBtn = document.querySelector(".hero-btn");
  const featuresSection = document.querySelector(".features");
  const navLinks = document.querySelector(".nav-links");
  const menuToggle = document.querySelector(".menu-toggle"); // ✅ toggle button
  const loginBtn = document.querySelector("header .btn");

  /* ---------------------------------------------------
     🌿 HERO SCROLL
  --------------------------------------------------- */
  if (heroBtn && featuresSection) {
    heroBtn.addEventListener("click", (e) => {
      e.preventDefault();
      featuresSection.scrollIntoView({ behavior: "smooth" });
    });
  }

  /* ---------------------------------------------------
     🌿 CONTACT / ABOUT / SERVICES POPUPS
  --------------------------------------------------- */
  const popupLinks = [
    { linkId: "about-link", tabId: "about-tab" },
    { linkId: "services-link", tabId: "services-tab" },
    { linkId: "contact-link", tabId: "contact-tab" },
  ];

  popupLinks.forEach((item) => {
    const link = document.getElementById(item.linkId);
    const tab = document.getElementById(item.tabId);
    if (link && tab) {
      link.addEventListener("click", (e) => {
        e.preventDefault();
        tab.classList.add("show");
        if (navLinks) navLinks.classList.remove("active"); // optional
      });
    }
  });

  const closeBtns = document.querySelectorAll(".close-tab");
  closeBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      const popup = btn.closest(".tab-popup");
      if (popup) popup.classList.remove("show");
    });
  });

  // Close tab when clicking outside
  window.addEventListener("click", (e) => {
    if (e.target.classList.contains("tab-popup")) {
      e.target.classList.remove("show");
    }
  });

  /* ---------------------------------------------------
     🌿 MOBILE MENU TOGGLE (click to open/close)
  --------------------------------------------------- */
  if (menuToggle && navLinks) {
    menuToggle.addEventListener("click", () => {
      navLinks.classList.toggle("active"); // ✅ open or close
      menuToggle.classList.toggle("open"); // optional — if you want to animate the icon
    });
  }

  /* ---------------------------------------------------
     🌿 LOGIN BUTTON IN HEADER
  --------------------------------------------------- */
  if (loginBtn) {
    loginBtn.addEventListener("click", () => {
      window.location.href = "/login";
    });
  }

  /* ---------------------------------------------------
     🌿 SCROLL TO TOP
  --------------------------------------------------- */
  const homeLink = document.getElementById("home-link");
  if (homeLink) {
    homeLink.addEventListener("click", (e) => {
      e.preventDefault();
      window.scrollTo({ top: 0, behavior: "smooth" });
    });
  }

  /* ---------------------------------------------------
     🌿 FORM TOGGLE (OPTIONAL LOGIN/SIGNUP)
  --------------------------------------------------- */
  const toggleForm = document.getElementById("toggle-form");
  const loginForm = document.getElementById("login-form");
  const signupForm = document.getElementById("signup-form");
  const formTitle = document.getElementById("form-title");
  const formSubtitle = document.getElementById("form-subtitle");

  if (toggleForm && loginForm && signupForm) {
    toggleForm.addEventListener("click", () => {
      loginForm.classList.toggle("hidden");
      signupForm.classList.toggle("hidden");

      if (loginForm.classList.contains("hidden")) {
        formTitle.textContent = "Sign Up for Skillvine";
        formSubtitle.innerHTML =
          'Already have an account? <span class="toggle-form">Login</span>';
      } else {
        formTitle.textContent = "Login to Skillvine";
        formSubtitle.innerHTML =
          'Don\'t have an account? <span class="toggle-form">Sign Up</span>';
      }
    });

    // Event delegation for dynamically added toggle spans
    document.addEventListener("click", (e) => {
      if (e.target.matches(".toggle-form")) {
        toggleForm.click();
      }
    });
  }
});
