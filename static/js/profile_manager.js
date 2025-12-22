async function api(path, opts={}){
  const res = await fetch(path, opts);
  if(!res.ok){
    const t = await res.text();
    throw new Error(`${res.status} ${t}`);
  }
  const ct = res.headers.get('content-type')||'';
  if(ct.includes('application/json')) return res.json();
  return res.text();
}

async function loadProfile(){
  const p = await api('/api/profile');
  document.getElementById('email_box').textContent = p.email + (p.email_verified? ' (verified)':' (unverified)');
  document.getElementById('full_name').value = p.full_name || '';
  document.getElementById('tv_status').textContent = p.teacher_verification || 'Not Requested';
  document.getElementById('view_as_select').value = (p.view_as || p.role || 'student');
}

async function sendName(){
  const name = document.getElementById('full_name').value;
  const res = await api('/api/change_name', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({full_name:name})});
  alert('Name saved');
  await loadProfile();
}

async function sendPassword(){
  const oldp = document.getElementById('old_password').value;
  const newp = document.getElementById('new_password').value;
  const res = await api('/api/change_password', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({old_password:oldp, new_password:newp})});
  alert('Password changed');
}

async function sendVerifyEmail(){
  await api('/api/request_email_verification', {method:'POST'});
  alert('Verification email requested. Check logs if email not sent.');
}

async function uploadId(e){
  e.preventDefault();
  const f = document.getElementById('id_file').files[0];
  if(!f) return alert('Choose a file');
  const form = new FormData();
  form.append('id_file', f);
  const res = await fetch('/api/upload_id', {method:'POST', body:form});
  if(!res.ok){ alert('Upload failed: '+await res.text()); return }
  alert('Uploaded. Verification will be processed by admin.');
  await loadProfile();
}

async function applyViewAs(save=false){
  const v = document.getElementById('view_as_select').value;
  await api('/api/view_as', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({view_as:v, save})});
  alert('View-as updated'+(save?' and saved':''));
  await loadProfile();
}

window.addEventListener('load', ()=>{
  loadProfile().catch(e=>console.error(e));
  document.getElementById('save_name').addEventListener('click', sendName);
  document.getElementById('save_password').addEventListener('click', sendPassword);
  document.getElementById('verify_email_btn').addEventListener('click', sendVerifyEmail);
  document.getElementById('upload_id_form').addEventListener('submit', uploadId);
  document.getElementById('apply_view_as').addEventListener('click', ()=>applyViewAs(false));
  document.getElementById('save_role').addEventListener('click', ()=>applyViewAs(true));
});
